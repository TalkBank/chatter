/*
 * Contract harness for send2clan's Windows executable-path admission.
 *
 * Compiles the real csrc/send2clan.c with _WIN32 selected against the
 * deterministic doubles in this directory (windows.h, shellapi.h), then drives
 * the public is_clan_available() and launchCLANcApp() entry points through
 * scripted registry states. Each scenario checks the observable consequences:
 * how many file probes and launches happened, and with exactly which path.
 *
 * The central contract: when the configured location cannot be admitted, the
 * bridge performs NO file probe and NO launch. Failed registry queries fill the
 * caller's buffer with garbage, modelling RegQueryValueExA's "contents
 * undefined" behavior, so any read of an unadmitted buffer shows up as a probe.
 *
 * This verifies the bridge's own logic only. It is not native Windows,
 * registry, ShellExecute or CLAN GUI qualification.
 */

/* System headers first, while the host's own platform macros are intact. */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Select the bridge's Windows branch. The host compiler predefines __APPLE__
 * on macOS, and the bridge tests __APPLE__ before _WIN32. */
#undef __APPLE__
#ifndef _WIN32
#define _WIN32 1
#endif
#define SEND2CLAN_STATIC_BUILD 1

#include "../../csrc/send2clan.c"

/* ------------------------------------------------------------------------ */
/* Scripted registry state                                                   */
/* ------------------------------------------------------------------------ */

/* How RegOpenKeyExA answers. */
typedef enum {
    KEY_ABSENT,       /* ERROR_FILE_NOT_FOUND */
    KEY_ACCESS_DENIED,/* ERROR_ACCESS_DENIED */
    KEY_OPENS         /* ERROR_SUCCESS; the value script then applies */
} key_script;

/* How RegQueryValueExA answers once the key is open. */
typedef enum {
    VALUE_MISSING,    /* ERROR_FILE_NOT_FOUND, buffer left as garbage */
    VALUE_MORE_DATA,  /* ERROR_MORE_DATA, buffer left as garbage */
    VALUE_STORED      /* ERROR_SUCCESS with `type` and exactly `bytes` stored */
} value_script;

typedef struct {
    key_script key;
    value_script value;
    DWORD type;
    const char *bytes; /* stored verbatim, NULs included */
    size_t byte_count;
    bool file_exists;  /* GetFileAttributesA answer for any probed path */
} registry_script;

static registry_script script;

/* ------------------------------------------------------------------------ */
/* Recorded effects                                                          */
/* ------------------------------------------------------------------------ */

typedef struct {
    int opens;
    int closes;
    int probes;
    int launches;
    char probed_path[MAX_PATH + 1];
    char launched_path[MAX_PATH + 1];
} effects;

static effects seen;

static struct win32_double_key *const OPEN_KEY = (struct win32_double_key *)(uintptr_t)0x1234u;
static struct win32_double_instance *const LAUNCHED = (struct win32_double_instance *)(uintptr_t)42u;

static void record_path(char *destination, LPCSTR path) {
    /* The bridge must hand over a NUL-terminated path within MAX_PATH. */
    size_t length = strnlen(path, MAX_PATH + 1);
    if (length > MAX_PATH - 1) {
        length = MAX_PATH;
    }
    memcpy(destination, path, length);
    destination[length] = '\0';
}

/* ------------------------------------------------------------------------ */
/* Win32 doubles                                                             */
/* ------------------------------------------------------------------------ */

HWND FindWindowA(LPCSTR class_name, LPCSTR window_name) {
    (void)class_name;
    (void)window_name;
    return NULL; /* CLAN is never already running in these scenarios */
}

LONG RegOpenKeyExA(HKEY key, LPCSTR sub_key, DWORD options, REGSAM desired, HKEY *result) {
    (void)key;
    (void)sub_key;
    (void)options;
    (void)desired;
    switch (script.key) {
        case KEY_ABSENT:
            return ERROR_FILE_NOT_FOUND;
        case KEY_ACCESS_DENIED:
            return ERROR_ACCESS_DENIED;
        case KEY_OPENS:
            seen.opens++;
            *result = OPEN_KEY;
            return ERROR_SUCCESS;
    }
    return ERROR_ACCESS_DENIED;
}

LONG RegQueryValueExA(HKEY key, LPCSTR value_name, DWORD *reserved, DWORD *type,
                      LPBYTE data, DWORD *data_size) {
    (void)value_name;
    (void)reserved;
    if (key != OPEN_KEY) {
        return ERROR_ACCESS_DENIED;
    }
    switch (script.value) {
        case VALUE_MISSING:
            memset(data, 'G', *data_size);
            return ERROR_FILE_NOT_FOUND;
        case VALUE_MORE_DATA:
            memset(data, 'G', *data_size);
            return ERROR_MORE_DATA;
        case VALUE_STORED:
            if (script.byte_count > *data_size) {
                memset(data, 'G', *data_size);
                *data_size = (DWORD)script.byte_count;
                return ERROR_MORE_DATA;
            }
            memset(data, 'G', *data_size);
            memcpy(data, script.bytes, script.byte_count);
            *data_size = (DWORD)script.byte_count;
            if (type) {
                *type = script.type;
            }
            return ERROR_SUCCESS;
    }
    return ERROR_ACCESS_DENIED;
}

LONG RegCloseKey(HKEY key) {
    if (key == OPEN_KEY) {
        seen.closes++;
    }
    return ERROR_SUCCESS;
}

DWORD GetFileAttributesA(LPCSTR path) {
    seen.probes++;
    record_path(seen.probed_path, path);
    return script.file_exists ? 0x20u : INVALID_FILE_ATTRIBUTES;
}

HINSTANCE ShellExecuteA(HWND window, LPCSTR operation, LPCSTR file, LPCSTR parameters,
                        LPCSTR directory, int show) {
    (void)window;
    (void)operation;
    (void)parameters;
    (void)directory;
    (void)show;
    seen.launches++;
    record_path(seen.launched_path, file);
    return LAUNCHED;
}

void Sleep(DWORD milliseconds) {
    (void)milliseconds;
}

DWORD GetTickCount(void) {
    return 0;
}

DWORD GetEnvironmentVariableA(LPCSTR name, char *buffer, DWORD size) {
    (void)name;
    (void)buffer;
    (void)size;
    return 0;
}

LRESULT SendMessageA(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    (void)window;
    (void)message;
    (void)wparam;
    (void)lparam;
    return 0;
}

int fopen_s(FILE **stream, const char *path, const char *mode) {
    (void)path;
    (void)mode;
    *stream = NULL;
    return 1;
}

/* Bounded CRT doubles with MSVC's failure behavior: on overflow the
 * destination becomes the empty string and a nonzero error is returned. */
int strcpy_s(char *destination, size_t size, const char *source) {
    size_t length = strlen(source);
    if (length + 1 > size) {
        if (size > 0) {
            destination[0] = '\0';
        }
        return 34; /* ERANGE */
    }
    memcpy(destination, source, length + 1);
    return 0;
}

int strcat_s(char *destination, size_t size, const char *source) {
    size_t existing = strnlen(destination, size);
    if (existing == size || existing + strlen(source) + 1 > size) {
        if (size > 0) {
            destination[0] = '\0';
        }
        return 34; /* ERANGE */
    }
    memcpy(destination + existing, source, strlen(source) + 1);
    return 0;
}

/* ------------------------------------------------------------------------ */
/* Scenarios                                                                 */
/* ------------------------------------------------------------------------ */

static int failures = 0;
static int scenarios = 0;

#define CHECK(scenario, condition)                                              \
    do {                                                                        \
        if (!(condition)) {                                                     \
            fprintf(stderr, "FAIL [%s] %s (line %d)\n", (scenario), #condition, \
                    __LINE__);                                                  \
            failures++;                                                         \
        }                                                                       \
    } while (0)

/* What the bridge should do with a scripted registry state. */
typedef enum {
    EXPECT_REFUSED,      /* no probe, no launch; unavailable, APP_NOT_FOUND */
    EXPECT_PROBES_PATH   /* probes and launches exactly `path` */
} expectation;

static void run(const char *name, registry_script given, expectation expected, const char *path) {
    scenarios++;

    /* Availability check. */
    script = given;
    memset(&seen, 0, sizeof(seen));
    bool available = is_clan_available();
    CHECK(name, seen.opens == seen.closes);
    switch (expected) {
        case EXPECT_REFUSED:
            CHECK(name, !available);
            CHECK(name, seen.probes == 0);
            break;
        case EXPECT_PROBES_PATH:
            CHECK(name, available == given.file_exists);
            CHECK(name, seen.probes == 1);
            CHECK(name, strcmp(seen.probed_path, path) == 0);
            break;
    }

    /* Launch. */
    script = given;
    memset(&seen, 0, sizeof(seen));
    int launched = launchCLANcApp();
    CHECK(name, seen.opens == seen.closes);
    switch (expected) {
        case EXPECT_REFUSED:
            CHECK(name, launched == SEND2CLAN_ERR_APP_NOT_FOUND);
            CHECK(name, seen.probes == 0);
            CHECK(name, seen.launches == 0);
            break;
        case EXPECT_PROBES_PATH:
            CHECK(name, seen.probes == 1);
            CHECK(name, strcmp(seen.probed_path, path) == 0);
            if (given.file_exists) {
                CHECK(name, launched == SEND2CLAN_SUCCESS);
                CHECK(name, seen.launches == 1);
                CHECK(name, strcmp(seen.launched_path, path) == 0);
            } else {
                CHECK(name, launched == SEND2CLAN_ERR_APP_NOT_FOUND);
                CHECK(name, seen.launches == 0);
            }
            break;
    }
}

static registry_script stored(DWORD type, const char *bytes, size_t byte_count) {
    registry_script s = {KEY_OPENS, VALUE_STORED, type, bytes, byte_count, true};
    return s;
}

int main(void) {
    static const char terminated[] = "C:\\Program Files\\CLAN";          /* sizeof keeps the NUL */
    static const char unterminated[] = {'C', ':', '\\', 'C', 'L', 'A', 'N'};
    static const char double_nul[] = "C:\\CLAN\0";                       /* two trailing NULs */
    static const char embedded_nul[] = {'C', ':', '\\', 'A', '\0', 'B'};
    static const char only_nul[] = {'\0'};

    /* Absent key: the compiled default, which the bridge probes. */
    registry_script absent = {KEY_ABSENT, VALUE_MISSING, 0, NULL, 0, true};
    run("absent key uses the default", absent, EXPECT_PROBES_PATH, "C:\\TalkBank\\CLAN\\CLAN.EXE");
    absent.file_exists = false;
    run("absent key, default missing", absent, EXPECT_PROBES_PATH, "C:\\TalkBank\\CLAN\\CLAN.EXE");

    /* Present key that cannot be opened: not silently the default. */
    registry_script denied = {KEY_ACCESS_DENIED, VALUE_MISSING, 0, NULL, 0, true};
    run("unreadable key is refused", denied, EXPECT_REFUSED, NULL);

    /* The original defect: key opens, query fails, buffer was uninitialized. */
    registry_script missing = {KEY_OPENS, VALUE_MISSING, 0, NULL, 0, true};
    run("failed value query is refused", missing, EXPECT_REFUSED, NULL);

    registry_script more_data = {KEY_OPENS, VALUE_MORE_DATA, 0, NULL, 0, true};
    run("ERROR_MORE_DATA is refused", more_data, EXPECT_REFUSED, NULL);

    run("REG_EXPAND_SZ is refused", stored(REG_EXPAND_SZ, terminated, sizeof(terminated)),
        EXPECT_REFUSED, NULL);
    run("REG_BINARY is refused", stored(REG_BINARY, terminated, sizeof(terminated)),
        EXPECT_REFUSED, NULL);
    run("zero-byte value is refused", stored(REG_SZ, only_nul, 0), EXPECT_REFUSED, NULL);
    run("NUL-only value is refused", stored(REG_SZ, only_nul, sizeof(only_nul)), EXPECT_REFUSED, NULL);
    run("embedded NUL is refused", stored(REG_SZ, embedded_nul, sizeof(embedded_nul)),
        EXPECT_REFUSED, NULL);

    run("terminated value resolves", stored(REG_SZ, terminated, sizeof(terminated)),
        EXPECT_PROBES_PATH, "C:\\Program Files\\CLAN\\CLAN.EXE");
    run("unterminated value resolves", stored(REG_SZ, unterminated, sizeof(unterminated)),
        EXPECT_PROBES_PATH, "C:\\CLAN\\CLAN.EXE");
    run("double trailing NUL resolves", stored(REG_SZ, double_nul, sizeof(double_nul)),
        EXPECT_PROBES_PATH, "C:\\CLAN\\CLAN.EXE");

    /* Capacity: value + "\\CLAN.EXE" (9 bytes) + NUL must fit in MAX_PATH. The
     * suffix is stated here independently: it is part of the contract. */
    static const char suffix[] = "\\CLAN.EXE";
    static char longest[MAX_PATH];
    static char expected_longest[MAX_PATH];
    const size_t fits = MAX_PATH - sizeof(suffix); /* 250 */
    memset(longest, 'x', sizeof(longest));
    memcpy(expected_longest, longest, fits);
    memcpy(expected_longest + fits, suffix, sizeof(suffix));
    run("longest unterminated value resolves", stored(REG_SZ, longest, fits), EXPECT_PROBES_PATH,
        expected_longest);
    run("one byte longer is refused", stored(REG_SZ, longest, fits + 1), EXPECT_REFUSED, NULL);
    run("full-buffer value is refused", stored(REG_SZ, longest, MAX_PATH), EXPECT_REFUSED, NULL);
    run("oversized value is refused", stored(REG_SZ, longest, MAX_PATH + 1), EXPECT_REFUSED, NULL);

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    printf("send2clan Win32 path admission: %d scenarios passed\n", scenarios);
    return 0;
}
