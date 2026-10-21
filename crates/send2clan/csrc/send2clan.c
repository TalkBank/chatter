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
    DWORD waitTime = timeOut > 0 ? (DWORD)(timeOut * 1000) : 4000; // Default 4 seconds
    Sleep(waitTime > 10000 ? 10000 : waitTime); // Cap at 10 seconds for launch wait
    return SEND2CLAN_SUCCESS;

#else
    (void)timeOut;  // Unused on Linux
    return SEND2CLAN_ERR_UNSUPPORTED_PLATFORM;
#endif
}

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
    char homeDirectory[256], messageFileName[512];
    FILE *messageFile;

    // Get user directory
    if (!GetEnvironmentVariableA("USERPROFILE", homeDirectory, sizeof(homeDirectory))) {
        const char *home = getenv("HOME");
        const char *user = getenv("USER");

        // Validate environment variables to prevent path injection
        if (home && user) {
            // Check for path traversal attempts in HOME
            if (strstr(home, "..") != NULL || strstr(home, "\\\\") != NULL) {
                return SEND2CLAN_ERR_SEND_FAILED;
            }
            // Check for path traversal attempts in USER
            if (strstr(user, "..") != NULL || strstr(user, "/") != NULL ||
                strstr(user, "\\") != NULL || strlen(user) > 64) {
                return SEND2CLAN_ERR_SEND_FAILED;
            }

            snprintf(homeDirectory, sizeof(homeDirectory), "%s/.wine/drive_c/users/%s", home, user);
        } else {
            return SEND2CLAN_ERR_SEND_FAILED;
        }
    }
    snprintf(messageFileName, sizeof(messageFileName), "%s%s", homeDirectory, MESSAGE_FILE_NAME);

    // Write message file
    if (fopen_s(&messageFile, messageFileName, "w") != 0 || !messageFile) {
        return SEND2CLAN_ERR_SEND_FAILED;
    }
    
    fprintf(messageFile, "%s\n", formattedMessage);
    fclose(messageFile);

    // Find CLAN window with timeout
    HWND window = NULL;
    DWORD startTime = GetTickCount();
    DWORD timeoutMs = timeOut > 0 ? (DWORD)(timeOut * 1000) : 30000; // Default 30 seconds
    
    while (!window && (GetTickCount() - startTime) < timeoutMs) {
        window = FindWindowA(MAIN_WINDOW_CLASS, NULL);
        if (!window) {
            Sleep(100); // Wait 100ms before retry
        }
    }
    
    if (!window) {
        return SEND2CLAN_ERR_TIMEOUT;
    }

    if (SendMessageA(window, WM_APP, 0, 0) != 0) {
        return SEND2CLAN_ERR_SEND_FAILED;
    }

    return SEND2CLAN_SUCCESS;

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
