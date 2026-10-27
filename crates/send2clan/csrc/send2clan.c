#include "send2clan.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#ifdef __APPLE__
#include <ApplicationServices/ApplicationServices.h>
#include <CoreServices/CoreServices.h>
#elif defined(_WIN32)
#include <windows.h>
#include <shellapi.h>
#endif

/*
 * Simple API Implementation
 * ========================
 * This implementation provides the ultra-simple API specified in requirements.
 * No context management, no complex structures, just simple integer error codes.
 */

// Error codes are now defined in send2clan.h

// Platform-specific constants
#ifdef __APPLE__
#define CLANC_BUNDLE_ID CFSTR("CLANc")
#define CLANC_CREATOR_CODE 'MCed'
#endif

#ifdef _WIN32
#define MAIN_WINDOW_CLASS "AfxClanAppClassName"
#define MESSAGE_FILE_NAME "\\CLAN_Message.txt"
#define DEFAULT_CLAN_PATH "C:\\TalkBank\\CLAN\\CLAN.EXE"
#endif

/*
 * Simple Helper Functions
 * ======================
 */

// Simple parameter validation
static int validate_parameters(const char* filePath, int lineNumber, int columnNumber) {
    // Basic NULL and empty string checks
    if (!filePath || strlen(filePath) == 0) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    // Enhanced path traversal protection
    // Check for literal ".." sequences
    if (strstr(filePath, "..") != NULL) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    // Check for URL-encoded traversal attempts (%2e%2e, %2E%2E, etc.)
    if (strstr(filePath, "%2e") != NULL || strstr(filePath, "%2E") != NULL) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    // Check for null bytes (path truncation attacks)
    if (strchr(filePath, '\0') != filePath + strlen(filePath)) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    // Reject excessively long paths (DoS prevention)
    if (strlen(filePath) > 4096) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    // Validate line and column numbers
    if (lineNumber < 1 || lineNumber > 1000000) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    if (columnNumber < 1 || columnNumber > 10000) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    return SEND2CLAN_SUCCESS;
}

// Simple platform error mapping (only used on macOS and Windows)
#if defined(__APPLE__) || defined(_WIN32)
static int map_platform_error(int platform_error) {
#ifdef __APPLE__
    // Map OSStatus codes to simple error codes
    switch (platform_error) {
        case 0: // noErr
            return SEND2CLAN_SUCCESS;
        case -600: // procNotFound
        case -609: // connectionInvalid
            return SEND2CLAN_ERR_APP_NOT_FOUND;
        case -1712: // errAETimeout
            return SEND2CLAN_ERR_TIMEOUT;
        case -108: // memFullErr
        case -50: // paramErr
            return SEND2CLAN_ERR_SEND_FAILED;
        default:
            return SEND2CLAN_ERR_SEND_FAILED;
    }
#elif defined(_WIN32)
    // Map Windows error codes to simple error codes
    if (platform_error == 0) {
        return SEND2CLAN_SUCCESS;
    }
    
    // Handle ShellExecute return values
    if (platform_error > 0 && platform_error <= 32) {
        switch (platform_error) {
            case 2: // ERROR_FILE_NOT_FOUND
            case 3: // ERROR_PATH_NOT_FOUND
                return SEND2CLAN_ERR_APP_NOT_FOUND;
            case 5: // ERROR_ACCESS_DENIED
                return SEND2CLAN_ERR_LAUNCH_FAILED;
            default:
                return SEND2CLAN_ERR_LAUNCH_FAILED;
        }
    }
    
    // Handle other Windows errors
    switch (platform_error) {
        case 258: // ERROR_TIMEOUT
            return SEND2CLAN_ERR_TIMEOUT;
        default:
            return SEND2CLAN_ERR_SEND_FAILED;
    }
#endif
}
#endif  // defined(__APPLE__) || defined(_WIN32)

#ifdef _WIN32
/*
 * Executable-path admission
 * =========================
 * A clan_executable_path holds a NUL-terminated path that
 * resolve_clan_executable has admitted, and it is produced nowhere else. Both
 * launch_clan_app and is_clan_available obtain it through the resolver and read
 * `value` only when clan_path_is_resolved accepts the outcome, so a failed or
 * malformed registry value can never reach a file probe or a launch. C cannot
 * make the field private; the single constructor, the closed outcome enum and
 * the exhaustive switch below are the discipline that stands in for a newtype.
 *
 * RegQueryValueExA's contract matters here: a successful string query need not
 * store a terminating NUL, the reported size counts bytes actually stored, and
 * ERROR_MORE_DATA leaves the buffer contents undefined. Each case is admitted
 * explicitly instead of being trusted.
 */
typedef struct {
    char value[MAX_PATH];
} clan_executable_path;

/* Every way resolution can end. Only the two RESOLVED_* outcomes fill the path. */
typedef enum {
    CLAN_PATH_RESOLVED_FROM_REGISTRY, /* InstallPath admitted, "\\CLAN.EXE" appended */
    CLAN_PATH_RESOLVED_DEFAULT,       /* the TalkBank key does not exist: compiled default */
    CLAN_PATH_KEY_UNREADABLE,         /* the key exists but could not be opened */
    CLAN_PATH_VALUE_UNREADABLE,       /* the key opened but InstallPath could not be read */
    CLAN_PATH_VALUE_NOT_STRING,       /* InstallPath is not REG_SZ */
    CLAN_PATH_VALUE_EMPTY,            /* InstallPath holds no characters */
    CLAN_PATH_VALUE_EMBEDDED_NUL,     /* a NUL precedes other characters */
    CLAN_PATH_TOO_LONG                /* the value, or value plus "\\CLAN.EXE", exceeds MAX_PATH */
} clan_path_resolution;

#define CLAN_REGISTRY_KEY "Software\\TalkBank\\CLAN"
#define CLAN_REGISTRY_VALUE "InstallPath"
#define CLAN_EXECUTABLE_SUFFIX "\\CLAN.EXE"

/* Fill `out` only on a RESOLVED_* outcome; on every other outcome `out` is untouched. */
static clan_path_resolution resolve_clan_executable(clan_executable_path *out) {
    HKEY key;
    LONG opened = RegOpenKeyExA(HKEY_LOCAL_MACHINE, CLAN_REGISTRY_KEY, 0, KEY_READ, &key);
    if (opened == ERROR_FILE_NOT_FOUND) {
        // No TalkBank key at all: the installer never registered a location.
        memcpy(out->value, DEFAULT_CLAN_PATH, sizeof(DEFAULT_CLAN_PATH));
        return CLAN_PATH_RESOLVED_DEFAULT;
    }
    if (opened != ERROR_SUCCESS) {
        return CLAN_PATH_KEY_UNREADABLE;
    }

    char raw[MAX_PATH];
    DWORD type = 0;
    DWORD stored = sizeof(raw);
    LONG queried = RegQueryValueExA(key, CLAN_REGISTRY_VALUE, NULL, &type, (LPBYTE)raw, &stored);
    RegCloseKey(key);
    if (queried == ERROR_MORE_DATA) {
        return CLAN_PATH_TOO_LONG;
    }
    if (queried != ERROR_SUCCESS) {
        return CLAN_PATH_VALUE_UNREADABLE;
    }
    if (type != REG_SZ) {
        return CLAN_PATH_VALUE_NOT_STRING;
    }
    if (stored > sizeof(raw)) {
        return CLAN_PATH_TOO_LONG;
    }

    // `stored` counts bytes written. A trailing run of NULs is terminator
    // padding; any NUL before the last character is an embedded NUL.
    size_t length = stored;
    while (length > 0 && raw[length - 1] == '\0') {
        length--;
    }
    if (length == 0) {
        return CLAN_PATH_VALUE_EMPTY;
    }
    if (memchr(raw, '\0', length) != NULL) {
        return CLAN_PATH_VALUE_EMBEDDED_NUL;
    }
    // sizeof includes the suffix's own NUL, which becomes the path's terminator.
    if (length + sizeof(CLAN_EXECUTABLE_SUFFIX) > sizeof(out->value)) {
        return CLAN_PATH_TOO_LONG;
    }
    memcpy(out->value, raw, length);
    memcpy(out->value + length, CLAN_EXECUTABLE_SUFFIX, sizeof(CLAN_EXECUTABLE_SUFFIX));
    return CLAN_PATH_RESOLVED_FROM_REGISTRY;
}

/* The one place that decides whether an outcome carries a usable path. No
 * default case: a new outcome must be classified here, and -Wswitch says so. */
static bool clan_path_is_resolved(clan_path_resolution resolution) {
    switch (resolution) {
        case CLAN_PATH_RESOLVED_FROM_REGISTRY:
        case CLAN_PATH_RESOLVED_DEFAULT:
            return true;
        case CLAN_PATH_KEY_UNREADABLE:
        case CLAN_PATH_VALUE_UNREADABLE:
        case CLAN_PATH_VALUE_NOT_STRING:
        case CLAN_PATH_VALUE_EMPTY:
        case CLAN_PATH_VALUE_EMBEDDED_NUL:
        case CLAN_PATH_TOO_LONG:
            return false;
    }
    return false;
}
#endif  // _WIN32

// Simple platform-specific launch function
static int launch_clan_app(long timeOut) {
#ifdef __APPLE__
    // Use timeout parameter to avoid unused parameter warning
    (void)timeOut; // Timeout not used in macOS launch, but parameter is required for API consistency
    
    CFArrayRef appURLs = LSCopyApplicationURLsForBundleIdentifier(CLANC_BUNDLE_ID, NULL);
    if (!appURLs || CFArrayGetCount(appURLs) == 0) {
        if (appURLs) CFRelease(appURLs);
        return SEND2CLAN_ERR_APP_NOT_FOUND;
    }

    CFURLRef appURL = CFArrayGetValueAtIndex(appURLs, 0);
    LSLaunchURLSpec launchSpec = {
        .appURL = appURL,
        .itemURLs = NULL,
        .passThruParams = NULL,
        .launchFlags = kLSLaunchDefaults,
        .asyncRefCon = NULL
    };

    OSStatus status = LSOpenFromURLSpec(&launchSpec, NULL);
    CFRelease(appURLs);

    return map_platform_error((int)status);
    
#elif defined(_WIN32)
    // Check if already running
    HWND window = FindWindowA(MAIN_WINDOW_CLASS, NULL);
    if (window) {
        return SEND2CLAN_SUCCESS;
    }

    // An unresolvable or malformed configured location is reported as "not
    // found" without probing or launching anything.
    clan_executable_path clanPath;
    if (!clan_path_is_resolved(resolve_clan_executable(&clanPath))) {
        return SEND2CLAN_ERR_APP_NOT_FOUND;
    }

    // Verify executable exists
    if (GetFileAttributesA(clanPath.value) == INVALID_FILE_ATTRIBUTES) {
        return SEND2CLAN_ERR_APP_NOT_FOUND;
    }

    // Launch the app
    HINSTANCE result = ShellExecuteA(NULL, NULL, clanPath.value, NULL, NULL, SW_SHOWNORMAL);
    if ((intptr_t)result <= 32) {
        return map_platform_error((int)(intptr_t)result);
    }

    // Wait for window to appear (use timeout)
    DWORD waitTime = timeOut <= 0 ? 4000 : timeOut >= 10 ? 10000 : (DWORD)timeOut * 1000u;
    Sleep(waitTime); // Clamp seconds before multiplying, including on 32-bit long.
    return SEND2CLAN_SUCCESS;

#else
    (void)timeOut;  // Unused on Linux
    return SEND2CLAN_ERR_UNSUPPORTED_PLATFORM;
#endif
}

#ifdef _WIN32
/* Only the resolver issues a complete destination to the file writer. */
typedef struct { char value[512]; } clan_message_path;
typedef enum { CLAN_DESTINATION_REFUSED, CLAN_DESTINATION_RESOLVED } clan_destination_kind;
typedef struct {
    clan_destination_kind kind;
    union { clan_message_path path; } admitted;
} clan_destination;

static clan_destination resolve_message_destination(void) {
    char directory[256];
    DWORD length = GetEnvironmentVariableA("USERPROFILE", directory, sizeof(directory));
    if (length >= sizeof(directory)) {
        /* Windows leaves the buffer undefined when it is too small. */
        return (clan_destination){.kind = CLAN_DESTINATION_REFUSED};
    }
    if (length == 0) {
        const char *home = getenv("HOME");
        const char *user = getenv("USER");
        if (!home || !user || strstr(home, "..") || strstr(home, "\\\\") ||
            strstr(user, "..") || strchr(user, '/') || strchr(user, '\\') || strlen(user) > 64) {
            return (clan_destination){.kind = CLAN_DESTINATION_REFUSED};
        }
        int written = snprintf(directory, sizeof(directory), "%s/.wine/drive_c/users/%s", home, user);
        if (written < 0 || (size_t)written >= sizeof(directory)) {
            return (clan_destination){.kind = CLAN_DESTINATION_REFUSED};
        }
    }
    clan_message_path path;
    int written = snprintf(path.value, sizeof(path.value), "%s%s", directory, MESSAGE_FILE_NAME);
    if (written < 0 || (size_t)written >= sizeof(path.value)) {
        return (clan_destination){.kind = CLAN_DESTINATION_REFUSED};
    }
    return (clan_destination){.kind = CLAN_DESTINATION_RESOLVED, .admitted.path = path};
}

typedef enum { CLAN_MESSAGE_REFUSED, CLAN_MESSAGE_PUBLISHED } clan_message_publication;

/* Publication owns the stream until close, including on a failed write. Only
 * the published outcome permits notification; failure may leave partial bytes. */
static clan_message_publication publish_windows_message(const clan_message_path *destination, const char *formattedMessage) {
    FILE *messageFile;
    if (fopen_s(&messageFile, destination->value, "w") != 0 || !messageFile) {
        return CLAN_MESSAGE_REFUSED;
    }
    int written = fprintf(messageFile, "%s\n", formattedMessage);
    int closed = fclose(messageFile);
    if (written < 0 || closed != 0) return CLAN_MESSAGE_REFUSED;
    return CLAN_MESSAGE_PUBLISHED;
}

/* Admission prevents overflow before publication. Only a ready target owns a
 * non-null window and a positive remaining notification budget. */
typedef struct { DWORD milliseconds; } clan_wait_duration;
typedef enum { CLAN_DURATION_REFUSED, CLAN_DURATION_ADMITTED } clan_duration_kind;
typedef struct {
    clan_duration_kind kind;
    union { clan_wait_duration duration; } admitted;
} clan_duration;
typedef struct { DWORD started; clan_wait_duration duration; } clan_wait_budget;
typedef struct { HWND window; UINT remaining; } clan_notification_target;
typedef enum { CLAN_WAIT_EXPIRED, CLAN_TARGET_READY } clan_target_kind;
typedef struct {
    clan_target_kind kind;
    union { clan_notification_target target; } admitted;
} clan_target;

static clan_duration admit_wait_duration(long seconds) {
    if (seconds > 0 && (unsigned long)seconds > UINT32_MAX / 1000u) {
        return (clan_duration){.kind = CLAN_DURATION_REFUSED};
    }
    return (clan_duration){.kind = CLAN_DURATION_ADMITTED,
        .admitted.duration = {seconds <= 0 ? 30000u : (DWORD)seconds * 1000u}};
}

static clan_target await_notification_target(clan_wait_budget budget) {
    for (;;) {
        HWND window = FindWindowA(MAIN_WINDOW_CLASS, NULL);
        /* Unsigned subtraction retains elapsed time across tick rollover. */
        DWORD elapsed = GetTickCount() - budget.started;
        if (elapsed >= budget.duration.milliseconds) {
            return (clan_target){.kind = CLAN_WAIT_EXPIRED};
        }
        DWORD remaining = budget.duration.milliseconds - elapsed;
        if (window) {
            return (clan_target){.kind = CLAN_TARGET_READY,
                .admitted.target = {window, remaining}};
        }
        Sleep(remaining < 100u ? remaining : 100u);
    }
}

static int notify_windows_target(clan_notification_target target) {
    DWORD_PTR reply;
    /* Failure need not set last error. Never interpret an old timeout as this
     * call's result, and never read the reply on transport failure. */
    SetLastError(ERROR_SUCCESS);
    if (!SendMessageTimeoutA(target.window, WM_APP, 0, 0,
            SMTO_BLOCK | SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT, target.remaining, &reply)) {
        return GetLastError() == ERROR_TIMEOUT ? SEND2CLAN_ERR_TIMEOUT : SEND2CLAN_ERR_SEND_FAILED;
    }
    return reply == 0 ? SEND2CLAN_SUCCESS : SEND2CLAN_ERR_SEND_FAILED;
}

static int send_windows_message(long timeOut, const clan_message_path *destination, const char *formattedMessage) {
    clan_duration duration = admit_wait_duration(timeOut);
    switch (duration.kind) {
        case CLAN_DURATION_REFUSED:
            return SEND2CLAN_ERR_INVALID_PARAMETER;
        case CLAN_DURATION_ADMITTED:
            break;
    }
    switch (publish_windows_message(destination, formattedMessage)) {
        case CLAN_MESSAGE_REFUSED:
            return SEND2CLAN_ERR_SEND_FAILED;
        case CLAN_MESSAGE_PUBLISHED:
            break;
    }
    clan_target target = await_notification_target((clan_wait_budget){GetTickCount(), duration.admitted.duration});
    switch (target.kind) {
        case CLAN_WAIT_EXPIRED:
            return SEND2CLAN_ERR_TIMEOUT;
        case CLAN_TARGET_READY:
            return notify_windows_target(target.admitted.target);
    }
    return SEND2CLAN_ERR_SEND_FAILED;
}

#endif

static int send_message_to_clan(long timeOut, const char* filePath, int lineNumber, int columnNumber, const char* message) {
    // Format the message. The buffer is sized generously for typical error
    // messages. If the message is longer (e.g., mor 1:1 alignment tables),
    // snprintf truncates it; CLAN seeing a truncated message is far better
    // than silently failing to send anything at all.
    char formattedMessage[8192];
    const char* msg = message ? message : "";
    snprintf(formattedMessage, sizeof(formattedMessage),
             "*** File \"%s\": line %d, column %d: %s",
             filePath, lineNumber, columnNumber, msg);

#ifdef __APPLE__
    AEDesc programDescriptor;
    AppleEvent event, reply;
    UInt32 signature = CLANC_CREATOR_CODE;

    AECreateDesc(typeApplSignature, &signature, sizeof(signature), &programDescriptor);
    AECreateAppleEvent(758934755, 0, &programDescriptor, kAutoGenerateReturnID, kAnyTransactionID, &event);
    AEPutParamPtr(&event, 1, typeChar, formattedMessage, strlen(formattedMessage) + 1);

    // Use provided timeout (convert to ticks, 60 ticks per second)
    long timeoutTicks = timeOut > 0 ? timeOut * 60 : 30 * 60; // Default 30 seconds
    OSErr err = AESendMessage(&event, &reply, kAEWaitReply | kAECanInteract | kAECanSwitchLayer, timeoutTicks);

    AEDisposeDesc(&programDescriptor);
    AEDisposeDesc(&event);
    AEDisposeDesc(&reply);

    return map_platform_error((int)err);
    
#elif defined(_WIN32)
    clan_destination destination = resolve_message_destination();
    switch (destination.kind) {
        case CLAN_DESTINATION_REFUSED:
            return SEND2CLAN_ERR_SEND_FAILED;
        case CLAN_DESTINATION_RESOLVED:
            return send_windows_message(timeOut, &destination.admitted.path, formattedMessage);
    }
    return SEND2CLAN_ERR_SEND_FAILED;

#else
    // Unused parameters on Linux
    (void)timeOut;
    (void)filePath;
    (void)lineNumber;
    (void)columnNumber;
    (void)message;
    return SEND2CLAN_ERR_UNSUPPORTED_PLATFORM;
#endif
}/*
 * 
Simple API Implementation
 * ========================
 */

// The one function that does everything as specified in requirements
SEND2CLAN_API int send2clan(long timeOut, const char *filePath, int lineNumber, int columnNumber, const char *message) {
    // Validate parameters
    int validation_result = validate_parameters(filePath, lineNumber, columnNumber);
    if (validation_result != SEND2CLAN_SUCCESS) {
        return validation_result;
    }
    
    // Launch CLAN if needed
    int launch_result = launch_clan_app(timeOut);
    if (launch_result != SEND2CLAN_SUCCESS) {
        return launch_result;
    }
    
    // Send the message
    return send_message_to_clan(timeOut, filePath, lineNumber, columnNumber, message);
}

// Optional helper function for advanced users
SEND2CLAN_API int launchCLANcApp(void) {
    return launch_clan_app(30); // Use default 30 second timeout
}

/*
 * Additional API Functions
 * =======================
 * These functions provide additional information and utilities for the library.
 */

// Simple version function
SEND2CLAN_API const char* send2clan_version(void) {
    return "1.0.0";
}

// Simple capabilities function
SEND2CLAN_API int send2clan_get_capabilities(uint32_t* capabilities) {
    if (!capabilities) {
        return SEND2CLAN_ERR_INVALID_PARAMETER;
    }

    *capabilities = 0;

    // Bit 0 (0x01): Platform supported
#if defined(__APPLE__) || defined(_WIN32)
    *capabilities |= 0x01;
#endif

    // Bit 1 (0x02): CLAN available
    if (is_clan_available()) {
        *capabilities |= 0x02;
    }

    // Bit 2 (0x04): Unicode support (always enabled)
    *capabilities |= 0x04;

    // Bit 3 (0x08): Timeout support (always enabled)
    *capabilities |= 0x08;

    // Bits 4-31: Reserved (always 0)

    return SEND2CLAN_SUCCESS;
}

// Simple platform support check
SEND2CLAN_API bool is_platform_supported(void) {
#ifdef __APPLE__
    return true;
#elif defined(_WIN32)
    return true;
#else
    return false;
#endif
}

// Simple CLAN availability check
SEND2CLAN_API bool is_clan_available(void) {
#ifdef __APPLE__
    CFArrayRef appURLs = LSCopyApplicationURLsForBundleIdentifier(CLANC_BUNDLE_ID, NULL);
    if (!appURLs || CFArrayGetCount(appURLs) == 0) {
        if (appURLs) CFRelease(appURLs);
        return false;
    }
    CFRelease(appURLs);
    return true;
#elif defined(_WIN32)
    clan_executable_path clanPath;
    if (!clan_path_is_resolved(resolve_clan_executable(&clanPath))) {
        return false;
    }

    // Check if executable exists
    return (GetFileAttributesA(clanPath.value) != INVALID_FILE_ATTRIBUTES);
#else
    return false;
#endif
}
