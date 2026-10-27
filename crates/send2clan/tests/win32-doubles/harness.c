/*
 * Contract harness for send2clan's Windows executable and message-path admission.
 *
 * Compiles the real csrc/send2clan.c with _WIN32 selected against the
 * deterministic doubles in this directory (windows.h, shellapi.h), then drives
 * the public is_clan_available() and launchCLANcApp() entry points through
 * scripted registry states. Each scenario checks the observable consequences:
 * how many file probes and launches happened, and with exactly which path.
 * Public send2clan() calls also check destination admission: oversized profile
 * and Wine fallback paths cannot reach the recorded file-open call.
 * Publication scenarios use anonymous temporary streams and scripted write/close
 * results. They check cleanup and notification order without opening the requested
 * path or invoking a real window.
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
#include <stdarg.h>
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

static char *scripted_getenv(const char *name);
static int scripted_fprintf(FILE *stream, const char *format, ...);
static int scripted_fclose(FILE *stream);
#define getenv scripted_getenv
#define fprintf scripted_fprintf
#define fclose scripted_fclose
#include "../../csrc/send2clan.c"
#undef getenv
#undef fprintf
#undef fclose

typedef enum { OPEN_REFUSED, WRITE_REFUSED, CLOSE_REFUSED, WRITE_AND_CLOSE_REFUSED, FILE_PUBLISHED } file_script;
static file_script file_behavior = OPEN_REFUSED;
typedef enum { WINDOW_ABSENT, WINDOW_PRESENT, WINDOW_UNTIL_PUBLICATION } window_script;
static window_script window_behavior = WINDOW_ABSENT;
static int message_writes, message_closes, notifications, early_notifications;
static char written_message[8192];
typedef enum { ACKNOWLEDGED, REJECTED_REPLY, TIMED_OUT, TRANSPORT_FAILED, FAILED_WITHOUT_ERROR } notification_script;
static notification_script notification_behavior = ACKNOWLEDGED;
static DWORD clock_tick, discovery_cost, last_error, last_sleep;
static UINT received_timeout, received_flags;
static int bounded_calls, unbounded_calls;


static int scripted_fprintf(FILE *stream, const char *format, ...) {
    message_writes++;
    va_list arguments;
    va_start(arguments, format);
    int length = vsnprintf(written_message, sizeof(written_message), format, arguments);
    va_end(arguments);
    switch (file_behavior) {
        case WRITE_REFUSED:
        case WRITE_AND_CLOSE_REFUSED:
            return -1;
        case CLOSE_REFUSED:
        case FILE_PUBLISHED:
            return fputs(written_message, stream) < 0 ? -1 : length;
        case OPEN_REFUSED:
            abort();
    }
    abort();
}

static int scripted_fclose(FILE *stream) {
    message_closes++;
    int result = fclose(stream);
    switch (file_behavior) {
        case CLOSE_REFUSED:
        case WRITE_AND_CLOSE_REFUSED:
            return EOF;
        case WRITE_REFUSED:
        case FILE_PUBLISHED:
            return result;
        case OPEN_REFUSED:
            abort();
    }
    abort();
}

typedef enum { PROFILE_VALUE, PROFILE_ABSENT } environment_kind;
typedef struct {
    environment_kind kind;
    union {
        const char *profile;
        struct { const char *home; const char *user; } fallback;
    } value;
} environment_script;

static environment_script environment = {PROFILE_ABSENT, {.fallback = {NULL, NULL}}};
static int message_file_opens;
static char message_path[512];

static char *scripted_getenv(const char *name) {
    if (environment.kind != PROFILE_ABSENT) return NULL;
    if (strcmp(name, "HOME") == 0) return (char *)environment.value.fallback.home;
    if (strcmp(name, "USER") == 0) return (char *)environment.value.fallback.user;
    return NULL;
}

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
    if (message_closes > 0) clock_tick += discovery_cost;
    switch (window_behavior) {
        case WINDOW_UNTIL_PUBLICATION:
            return message_closes == 0 ? (HWND)(uintptr_t)0x5678u : NULL;
        case WINDOW_ABSENT: return NULL;
        case WINDOW_PRESENT: return (HWND)(uintptr_t)0x5678u;
    }
    abort();
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
    last_sleep = milliseconds;
    clock_tick += milliseconds;
}

DWORD GetTickCount(void) {
    return clock_tick;
}

DWORD GetEnvironmentVariableA(LPCSTR name, char *buffer, DWORD size) {
    (void)name;
    switch (environment.kind) {
        case PROFILE_ABSENT: return 0;
        case PROFILE_VALUE: {
            size_t length = strlen(environment.value.profile);
            if (length >= size) {
                /* A too-small buffer is undefined, not a usable partial path. */
                memset(buffer, '!', size);
                if (size) buffer[size - 1] = '\0';
                return (DWORD)length + 1;
            }
            memcpy(buffer, environment.value.profile, length + 1);
            return (DWORD)length;
        }
    }
    abort();
}

LRESULT SendMessageA(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    (void)window;
    (void)message;
    (void)wparam;
    (void)lparam;
    unbounded_calls++;
    notifications++;
    if (message_closes != 1) early_notifications++;
    return 0;
}

void SetLastError(DWORD error) { last_error = error; }
DWORD GetLastError(void) { return last_error; }
LRESULT SendMessageTimeoutA(HWND window, UINT message, WPARAM wparam, LPARAM lparam,
                           UINT flags, UINT timeout, DWORD_PTR *reply) {
    if (window != (HWND)(uintptr_t)0x5678u || message != WM_APP || wparam || lparam) abort();
    notifications++;
    bounded_calls++;
    if (message_closes != 1) early_notifications++;
    received_timeout = timeout;
    received_flags = flags;
    switch (notification_behavior) {
        case ACKNOWLEDGED: *reply = 0; return 1;
        case REJECTED_REPLY: *reply = 7; return 1;
        case TIMED_OUT: last_error = ERROR_TIMEOUT; return 0;
        case TRANSPORT_FAILED: last_error = ERROR_ACCESS_DENIED; return 0;
        case FAILED_WITHOUT_ERROR: return 0;
    }
    abort();
}

int fopen_s(FILE **stream, const char *path, const char *mode) {
    message_file_opens++;
    snprintf(message_path, sizeof(message_path), "%s", path);
    (void)mode;
    switch (file_behavior) {
        case OPEN_REFUSED:
            *stream = NULL;
            return 1;
        case WRITE_REFUSED:
        case CLOSE_REFUSED:
        case WRITE_AND_CLOSE_REFUSED:
        case FILE_PUBLISHED:
            /* An anonymous test file; never open the requested destination. */
            *stream = tmpfile();
            return *stream ? 0 : 1;
    }
    abort();
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

typedef struct {
    expectation kind;
    union { const char *path; } admitted;
} destination_expectation;

static destination_expectation refuses_destination(void) {
    return (destination_expectation){.kind = EXPECT_REFUSED};
}

static destination_expectation opens_destination(const char *path) {
    return (destination_expectation){.kind = EXPECT_PROBES_PATH, .admitted.path = path};
}

/* Stop at file-open admission: no file, message or GUI is created. */
static void run_destination(const char *name, environment_script given, destination_expectation expected) {
    scenarios++;
    environment = given;
    script = (registry_script){KEY_ABSENT, VALUE_MISSING, 0, NULL, 0, true};
    memset(&seen, 0, sizeof(seen));
    message_file_opens = 0;
    message_path[0] = '\0';
    CHECK(name, send2clan(1, "sample.cha", 1, 1, NULL) == SEND2CLAN_ERR_SEND_FAILED);
    CHECK(name, seen.launches == 1);
    switch (expected.kind) {
        case EXPECT_REFUSED:
            CHECK(name, message_file_opens == 0);
            break;
        case EXPECT_PROBES_PATH:
            CHECK(name, message_file_opens == 1);
            CHECK(name, strcmp(message_path, expected.admitted.path) == 0);
            break;
    }
}

static void run_publication(const char *name, file_script given) {
    scenarios++;
    file_behavior = given;
    window_behavior = WINDOW_PRESENT;
    environment = (environment_script){PROFILE_VALUE, {.profile = "C:\\Users\\operator"}};
    message_file_opens = message_writes = message_closes = notifications = early_notifications = 0;
    written_message[0] = '\0';
    int result = send2clan(1, "sample.cha", 1, 1, "synthetic");
    CHECK(name, message_file_opens == 1);
    CHECK(name, early_notifications == 0);
    switch (given) {
        case OPEN_REFUSED:
            CHECK(name, result == SEND2CLAN_ERR_SEND_FAILED);
            CHECK(name, message_writes == 0 && message_closes == 0 && notifications == 0);
            break;
        case WRITE_REFUSED:
        case CLOSE_REFUSED:
        case WRITE_AND_CLOSE_REFUSED:
            CHECK(name, result == SEND2CLAN_ERR_SEND_FAILED);
            CHECK(name, message_writes == 1 && message_closes == 1 && notifications == 0);
            break;
        case FILE_PUBLISHED:
            CHECK(name, result == SEND2CLAN_SUCCESS);
            CHECK(name, message_writes == 1 && message_closes == 1 && notifications == 1);
            CHECK(name, strcmp(written_message, "*** File \"sample.cha\": line 1, column 1: synthetic\n") == 0);
            break;
    }
    file_behavior = OPEN_REFUSED;
    window_behavior = WINDOW_ABSENT;
}

typedef enum { EXPECT_NO_PUBLICATION, EXPECT_NO_NOTIFICATION, EXPECT_NOTIFICATION } notification_expectation;
static void run_notification(const char *name, long seconds, DWORD start, DWORD cost,
                             window_script window, notification_script behavior,
                             notification_expectation expected, int status, UINT remaining) {
    scenarios++;
    file_behavior = FILE_PUBLISHED;
    window_behavior = window;
    notification_behavior = behavior;
    environment = (environment_script){PROFILE_VALUE, {.profile = "C:\\Users\\operator"}};
    message_file_opens = message_writes = message_closes = notifications = early_notifications = 0;
    bounded_calls = unbounded_calls = 0;
    clock_tick = start;
    last_sleep = 0;
    discovery_cost = cost;
    last_error = ERROR_TIMEOUT; /* A failing API need not replace a stale error. */
    int result = send2clan(seconds, "sample.cha", 1, 1, "synthetic");
    CHECK(name, result == status);
    CHECK(name, unbounded_calls == 0 && early_notifications == 0);
    switch (expected) {
        case EXPECT_NO_PUBLICATION:
            CHECK(name, message_file_opens == 0 && notifications == 0);
            break;
        case EXPECT_NO_NOTIFICATION:
            CHECK(name, message_closes == 1 && notifications == 0);
            break;
        case EXPECT_NOTIFICATION:
            CHECK(name, message_closes == 1 && notifications == 1 && bounded_calls == 1);
            CHECK(name, received_timeout == remaining);
            CHECK(name, received_flags == (SMTO_BLOCK | SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT));
            break;
    }
    discovery_cost = 0;
    notification_behavior = ACKNOWLEDGED;
    file_behavior = OPEN_REFUSED;
    window_behavior = WINDOW_ABSENT;
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

    run_destination("profile path", (environment_script){PROFILE_VALUE, {.profile = "C:\\Users\\operator"}},
                    opens_destination("C:\\Users\\operator\\CLAN_Message.txt"));
    char profile[257], expected_profile[512];
    memset(profile, 'p', 255);
    profile[255] = '\0';
    snprintf(expected_profile, sizeof(expected_profile), "%s\\CLAN_Message.txt", profile);
    run_destination("largest profile", (environment_script){PROFILE_VALUE, {.profile = profile}}, opens_destination(expected_profile));
    profile[255] = 'p'; profile[256] = '\0';
    run_destination("oversized profile", (environment_script){PROFILE_VALUE, {.profile = profile}}, refuses_destination());
    run_destination("Wine fallback", (environment_script){PROFILE_ABSENT, {.fallback = {"/home/operator", "operator"}}},
                    opens_destination("/home/operator/.wine/drive_c/users/operator\\CLAN_Message.txt"));
    run_destination("missing fallback", (environment_script){PROFILE_ABSENT, {.fallback = {NULL, "operator"}}}, refuses_destination());
    run_destination("unsafe fallback", (environment_script){PROFILE_ABSENT, {.fallback = {"/home/../operator", "operator"}}}, refuses_destination());
    char home[256], expected_home[512];
    const size_t home_fits = 255 - strlen("/.wine/drive_c/users/u");
    memset(home, 'h', home_fits); home[home_fits] = '\0';
    snprintf(expected_home, sizeof(expected_home), "%s/.wine/drive_c/users/u\\CLAN_Message.txt", home);
    run_destination("largest fallback", (environment_script){PROFILE_ABSENT, {.fallback = {home, "u"}}}, opens_destination(expected_home));
    home[home_fits] = 'h'; home[home_fits + 1] = '\0';
    run_destination("oversized fallback", (environment_script){PROFILE_ABSENT, {.fallback = {home, "u"}}}, refuses_destination());

    run_publication("open refusal does not notify", OPEN_REFUSED);
    run_publication("write refusal closes without notification", WRITE_REFUSED);
    run_publication("close refusal does not notify", CLOSE_REFUSED);
    run_publication("write and close refusal do not notify", WRITE_AND_CLOSE_REFUSED);
    run_publication("complete publication notifies once", FILE_PUBLISHED);

    run_notification("lookup consumes budget", 1, 0, 100, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, 900);
    run_notification("late discovery expires", 1, 0, 1000, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NO_NOTIFICATION, SEND2CLAN_ERR_TIMEOUT, 0);
    run_notification("missing window expires", 1, 0, 0, WINDOW_UNTIL_PUBLICATION, ACKNOWLEDGED, EXPECT_NO_NOTIFICATION, SEND2CLAN_ERR_TIMEOUT, 0);
    run_notification("notification timeout", 1, 0, 0, WINDOW_PRESENT, TIMED_OUT, EXPECT_NOTIFICATION, SEND2CLAN_ERR_TIMEOUT, 1000);
    run_notification("transport failure", 1, 0, 0, WINDOW_PRESENT, TRANSPORT_FAILED, EXPECT_NOTIFICATION, SEND2CLAN_ERR_SEND_FAILED, 1000);
    run_notification("failure clears stale error", 1, 0, 0, WINDOW_PRESENT, FAILED_WITHOUT_ERROR, EXPECT_NOTIFICATION, SEND2CLAN_ERR_SEND_FAILED, 1000);
    run_notification("reply rejection", 1, 0, 0, WINDOW_PRESENT, REJECTED_REPLY, EXPECT_NOTIFICATION, SEND2CLAN_ERR_SEND_FAILED, 1000);
    run_notification("default zero", 0, 0, 0, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, 30000);
    run_notification("default negative", -1, 0, 0, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, 30000);
    run_notification("oversized duration", (long)(UINT32_MAX / 1000u) + 1, 0, 0, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NO_PUBLICATION, SEND2CLAN_ERR_INVALID_PARAMETER, 0);
    run_notification("largest duration", UINT32_MAX / 1000u, 0, 0, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, (UINT32_MAX / 1000u) * 1000u);
    run_notification("tick rollover", 1, UINT32_MAX - 50, 100, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, 900);
    run_notification("last short interval", 1, 0, 950, WINDOW_PRESENT, ACKNOWLEDGED, EXPECT_NOTIFICATION, SEND2CLAN_SUCCESS, 50);

    run_notification("lookup sleep uses remainder", 1, 0, 950, WINDOW_UNTIL_PUBLICATION, ACKNOWLEDGED, EXPECT_NO_NOTIFICATION, SEND2CLAN_ERR_TIMEOUT, 0);
    CHECK("lookup sleep uses remainder", last_sleep == 50);

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    printf("send2clan Win32 path admission: %d scenarios passed\n", scenarios);
    return 0;
}
