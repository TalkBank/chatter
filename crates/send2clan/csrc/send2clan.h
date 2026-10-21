/**
 * @file send2clan.h
 * @brief Ultra-simple library for sending file open messages to CLAN linguistic analysis software
 * 
 * send2clan is a C library that enables integration with CLAN (Computerized Language Analysis)
 * software by providing the ability to send file open messages with error information and cursor
 * positioning. The library supports both macOS (via Apple Events) and Windows (via message files
 * and window messaging).
 * 
 * @author send2clan development team
 * @version 1.0.0
 * @date 2025
 * 
 * @section example_usage Ultra-Simple Usage Example
 * 
 * @code{.c}
 * #include "send2clan.h"
 * 
 * int main() {
 *     // One function does everything: launch CLAN + send file + show error
 *     int result = send2clan(30, "/path/to/file.cha", 42, 15, "Syntax error");
 *     
 *     if (result != 0) {
 *         printf("send2clan failed with error code: %d\n", result);
 *         return 1;
 *     }
 *     
 *     printf("Successfully sent file to CLAN!\n");
 *     return 0;
 * }
 * @endcode
 * 
 * @section platform_support Platform Support
 * 
 * - **macOS**: Apple Events and Launch Services implementation
 * - **Windows**: Win32/message-file implementation; see integration limits below
 * - **Linux**: Returns non-zero error code (unsupported)
 * 
 * @section thread_safety Thread Safety
 * 
 * Windows sends share one message file without coordination, including across
 * processes. Concurrent sends are not guaranteed to preserve each request.
 * Installation queries, timeout flags and transport success do not establish
 * native platform qualification or successful GUI navigation.
 * 
 * @section memory_management Memory Management
 * 
 * No memory management required. Just call the function and check the return code.
 */

#ifndef SEND2CLAN_H
#define SEND2CLAN_H

// Standard boolean type
#ifndef __cplusplus
#include <stdbool.h>
#endif

// Standard integer types
#include <stdint.h>

/**
 * @defgroup error_codes Error Codes
 * @brief Return codes from send2clan library functions
 *
 * All error codes are stable across versions (ABI guarantee). These values will
 * never change in future releases to maintain binary compatibility.
 *
 * Error codes are designed to be:
 * - Deterministic: Same conditions always produce same error code
 * - Actionable: Each error suggests a specific recovery path
 * - Stable: Values fixed for ABI compatibility
 * @{
 */

/**
 * @brief Operation completed successfully
 *
 * The bridge reported transport success. This does not verify that CLAN opened
 * the intended file, positioned its cursor, or displayed the complete message.
 */
#define SEND2CLAN_SUCCESS 0

/**
 * @brief Platform is not supported
 *
 * The library only supports macOS and Windows. This error is returned on Linux
 * or other operating systems.
 *
 * Recovery: Check platform before calling with is_platform_supported()
 */
#define SEND2CLAN_ERR_UNSUPPORTED_PLATFORM 1

/**
 * @brief Failed to launch CLAN application
 *
 * The system call to launch CLAN (Launch Services on macOS, ShellExecute on
 * Windows) failed. The application may be installed but not launchable.
 *
 * Recovery: Check CLAN installation, file permissions, or system resources
 */
#define SEND2CLAN_ERR_LAUNCH_FAILED 2

/**
 * @brief CLAN application not found on system
 *
 * The library could not locate the CLAN application bundle (macOS) or executable
 * (Windows) on the system.
 *
 * macOS: Queries Launch Services with the implementation's "CLANc" identifier
 * Windows: The registered InstallPath (or the default path when no TalkBank
 * key exists) has no executable, or the registered value cannot be admitted
 *
 * Recovery: Install CLAN from https://dali.talkbank.org/clan/
 */
#define SEND2CLAN_ERR_APP_NOT_FOUND 3

/**
 * @brief Failed to send message to CLAN
 *
 * Communication with CLAN failed after successful launch. This could indicate:
 * - Apple Event delivery failure (macOS)
 * - Message file write failure (Windows)
 * - Window message posting failure (Windows)
 *
 * Recovery: Retry the operation; check CLAN is responsive; verify disk space
 */
#define SEND2CLAN_ERR_SEND_FAILED 4

/**
 * @brief Operation timed out
 *
 * A timeout-aware platform operation reported expiry. Not every operation is
 * bounded by timeOut; this code does not establish an end-to-end deadline.
 *
 * Recovery: Increase timeout value; check CLAN responsiveness; restart CLAN
 */
#define SEND2CLAN_ERR_TIMEOUT 5

/**
 * @brief Invalid parameter(s) provided
 *
 * One or more parameters failed validation:
 * - filePath is NULL or empty
 * - lineNumber < 1
 * - columnNumber < 1
 * - filePath contains invalid characters or path traversal attempts
 *
 * Recovery: Fix the calling code - this indicates a programming error
 */
#define SEND2CLAN_ERR_INVALID_PARAMETER 6

/**
 * @brief Unknown or unexpected error
 *
 * An error occurred that doesn't fit other categories. This should rarely happen
 * and likely indicates a bug in the library or an unexpected system condition.
 *
 * Recovery: Report as bug with details of system, parameters, and context
 */
#define SEND2CLAN_ERR_UNKNOWN 99

/** @} */ // end of error_codes group

/**
 * @brief Export macro for Windows DLL support
 */
#ifdef _WIN32
#    ifdef SEND2CLAN_SHARED_BUILD
#        define SEND2CLAN_API __declspec(dllexport)
#    elif defined(SEND2CLAN_STATIC_BUILD) || defined(SEND2CLAN_STATIC)
#        define SEND2CLAN_API
#    else
#        define SEND2CLAN_API __declspec(dllimport)
#    endif
#else
#    define SEND2CLAN_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Send file information to CLAN application
 *
 * This is the main function of the send2clan library. It performs a complete
 * workflow to communicate with CLAN:
 *
 * 1. **Parameter Validation**: Checks all parameters for validity
 * 2. **CLAN Launch**: Starts CLAN if not already running (platform-specific)
 * 3. **Message Delivery**: Sends file path, cursor position, and message to CLAN
 *
 * The function is **self-contained** - no initialization or cleanup required.
 * Windows callers share an uncoordinated message file. Do not overlap sends;
 * a process-local lock does not coordinate independent sending processes.
 *
 * @section platform_behavior Platform-Specific Behavior
 *
 * **macOS:**
 * - Uses Apple Events to communicate with CLAN
 * - Queries bundle ID "CLANc"; sends an Apple Event to creator code 'MCed'
 * - Uses Launch Services to start CLAN if needed
 * - Launch ignores timeOut; the Apple Event wait uses it
 *
 * **Windows:**
 * - Creates message file in user's home directory
 * - Uses synchronous SendMessageA with WM_APP to CLAN's main window
 * - Uses ShellExecute to start CLAN if needed
 * - Launch and window lookup wait separately; SendMessageA is not timeout-bounded
 *
 * @section usage_example Basic Usage Example
 *
 * @code{.c}
 * #include <send2clan/send2clan.h>
 * #include <stdio.h>
 *
 * int main(void) {
 *     // Check platform support first
 *     if (!is_platform_supported()) {
 *         fprintf(stderr, "send2clan only supports macOS and Windows\n");
 *         return 1;
 *     }
 *
 *     // Request a platform wait, not a whole-operation deadline
 *     int result = send2clan(
 *         30,                          // timeout in seconds
 *         "/Users/user/data/test.cha", // file path
 *         42,                          // line number (1-based)
 *         15,                          // column number (1-based)
 *         "Syntax error detected"      // optional message
 *     );
 *
 *     if (result == SEND2CLAN_SUCCESS) {
 *         printf("File sent to CLAN successfully!\n");
 *         return 0;
 *     } else {
 *         fprintf(stderr, "Error: %d\n", result);
 *         return 1;
 *     }
 * }
 * @endcode
 *
 * @section thread_safety Thread Safety
 *
 * No concurrent-delivery guarantee. Windows requests share CLAN_Message.txt.
 * Review coordination across all sending processes, not just threads in one caller.
 *
 * @section performance Performance Characteristics
 *
 * No latency bound is established. The combined formatted message can be
 * truncated by the fixed C buffer. Retrying may repeat partial external effects.
 *
 * @section memory_management Memory Management
 *
 * - **Platform resources**: Platform APIs may allocate resources internally
 * - **No cleanup required**: No resources to free after calling
 * - **String ownership**: All string parameters are read-only; caller retains ownership
 *
 * @param timeOut Seconds for selected platform waits, not the entire operation.
 *                Nonpositive values select platform-specific defaults.
 *
 * @param filePath Full path to the .cha file to open in CLAN. Must be:
 *                 - Non-NULL and non-empty
 *                 - Valid UTF-8 encoding
 *                 - Absolute or relative path (relative to CLAN's working directory)
 *                 - No path traversal attempts (../ rejected)
 *
 * @param lineNumber 1-based line number for cursor positioning. Must be >= 1.
 *                   CLAN will position the cursor at this line when opening the file.
 *
 * @param columnNumber 1-based column number for cursor positioning. Must be >= 1.
 *                     CLAN will position the cursor at this column within the line.
 *
 * @param message Optional error/status message to display in CLAN. Can be NULL.
 *                If provided, CLAN will display this message along with the file.
 *                Should be UTF-8 encoded. Typical use: error descriptions, warnings.
 *
 * @return SEND2CLAN_SUCCESS (0) on success
 * @return SEND2CLAN_ERR_UNSUPPORTED_PLATFORM (1) if platform is not macOS or Windows
 * @return SEND2CLAN_ERR_LAUNCH_FAILED (2) if CLAN launch attempt failed
 * @return SEND2CLAN_ERR_APP_NOT_FOUND (3) if CLAN not installed on system
 * @return SEND2CLAN_ERR_SEND_FAILED (4) if message delivery to CLAN failed
 * @return SEND2CLAN_ERR_TIMEOUT (5) if a timeout-aware wait reports expiry
 * @return SEND2CLAN_ERR_INVALID_PARAMETER (6) if any parameter fails validation
 * @return SEND2CLAN_ERR_UNKNOWN (99) for unexpected errors (report as bug)
 *
 * @see send2clan_version()
 * @see is_platform_supported()
 * @see is_clan_available()
 */
SEND2CLAN_API int send2clan(long timeOut, const char *filePath, int lineNumber, int columnNumber, const char *message);

/**
 * @brief Get library version string
 *
 * Returns the semantic version string of the send2clan library in the format
 * "MAJOR.MINOR.PATCH" (e.g., "1.0.0").
 *
 * The version follows Semantic Versioning 2.0.0:
 * - **MAJOR**: Incompatible API changes
 * - **MINOR**: Backward-compatible new features
 * - **PATCH**: Backward-compatible bug fixes
 *
 * @section usage_example Example
 *
 * @code{.c}
 * const char* version = send2clan_version();
 * printf("Using send2clan version %s\n", version);
 * // Output: "Using send2clan version 1.0.0"
 * @endcode
 *
 * @section memory_management Memory Management
 *
 * The returned string is statically allocated. **Do not free it.**
 * The pointer remains valid for the lifetime of the program.
 *
 * @section thread_safety Thread Safety
 *
 * **Thread-safe**: Can be called from multiple threads simultaneously.
 *
 * @return Pointer to static version string. Never returns NULL.
 *
 * @see send2clan_get_capabilities()
 */
SEND2CLAN_API const char* send2clan_version(void);

/**
 * @brief Get library capabilities as bit flags
 *
 * Returns legacy capability flags. Bits 2 and 3 are unconditional, including
 * on unsupported platforms: they do not test Unicode delivery or whole-operation
 * deadlines. Bit 1 inherits the installation-query limitations below.
 *
 * @section capability_bits Capability Bit Flags
 *
 * The capabilities bitmask uses the following bits:
 *
 * - **Bit 0 (0x01)**: Platform supported (macOS or Windows)
 * - **Bit 1 (0x02)**: CLAN application available on system
 * - **Bit 2 (0x04)**: Unicode path support (always enabled)
 * - **Bit 3 (0x08)**: Timeout support (always enabled)
 * - **Bits 4-31**: Reserved for future use (currently 0)
 *
 * @section usage_example Example Usage
 *
 * @code{.c}
 * uint32_t caps = 0;
 * int result = send2clan_get_capabilities(&caps);
 *
 * if (result != SEND2CLAN_SUCCESS) {
 *     fprintf(stderr, "Failed to query capabilities\n");
 *     return 1;
 * }
 *
 * if (caps & 0x01) {
 *     printf("Platform is supported\n");
 * }
 *
 * if (caps & 0x02) {
 *     printf("CLAN is installed\n");
 * } else {
 *     printf("CLAN not found - please install from https://dali.talkbank.org/clan/\n");
 * }
 *
 * if (caps & 0x04) {
 *     printf("Unicode paths supported\n");
 * }
 *
 * if (caps & 0x08) {
 *     printf("Timeout control supported\n");
 * }
 * @endcode
 *
 * @section typical_values Typical Capability Values
 *
 * - **macOS with CLAN**: 0x0F (all bits set)
 * - **macOS without CLAN**: 0x0D (bit 1 clear)
 * - **Windows with CLAN**: 0x0F (all bits set)
 * - **Windows without CLAN**: 0x0D (bit 1 clear)
 * - **Linux**: 0x0C (only Unicode and timeout bits)
 *
 * @section thread_safety Thread Safety
 *
 * **Thread-safe**: Multiple threads can call this function concurrently.
 *
 * @section performance Performance
 *
 * No latency bound is established for the installation query.
 *
 * @param capabilities Pointer to uint32_t where capability flags will be stored.
 *                     Must not be NULL. Previous value is overwritten.
 *
 * @return SEND2CLAN_SUCCESS (0) on success
 * @return SEND2CLAN_ERR_INVALID_PARAMETER (6) if capabilities is NULL
 *
 * @see is_platform_supported()
 * @see is_clan_available()
 */
SEND2CLAN_API int send2clan_get_capabilities(uint32_t* capabilities);

/**
 * @brief Check if current platform is supported by send2clan
 *
 * Performs a compile-time check to determine if the library was built for a
 * supported platform (macOS or Windows). This function always returns the same
 * value for a given binary.
 *
 * @section usage_example Example Usage
 *
 * @code{.c}
 * if (!is_platform_supported()) {
 *     fprintf(stderr, "Error: send2clan only supports macOS and Windows\n");
 *     fprintf(stderr, "This platform is not supported\n");
 *     return 1;
 * }
 *
 * // Target support does not guarantee successful delivery.
 * int result = send2clan(30, file, line, col, msg);
 * @endcode
 *
 * @section when_to_call When to Call This
 *
 * Call this function early in your application initialization to provide clear
 * error messages to users on unsupported platforms. This is preferable to
 * receiving SEND2CLAN_ERR_UNSUPPORTED_PLATFORM at runtime.
 *
 * @section thread_safety Thread Safety
 *
 * **Thread-safe**: Can be called from multiple threads simultaneously.
 *
 * @section performance Performance
 *
 * Returns a build-target constant; this is not a live capability test.
 *
 * @return true if platform is macOS or Windows
 * @return false if platform is Linux or other unsupported OS
 *
 * @see is_clan_available()
 * @see send2clan_get_capabilities()
 */
SEND2CLAN_API bool is_platform_supported(void);

/**
 * @brief Check if CLAN application is installed and available
 *
 * Performs a runtime check to determine if the CLAN application can be found
 * on the system. This is more expensive than is_platform_supported() as it may
 * involve filesystem or registry access.
 *
 * @section platform_behavior Platform-Specific Detection
 *
 * **macOS:**
 * - Queries bundle ID "CLANc" via Launch Services
 * - Returns true when the query returns at least one URL, not a launch proof
 *
 * **Windows:**
 * - Queries InstallPath in HKEY_LOCAL_MACHINE\\Software\\TalkBank\\CLAN
 * - Uses C:\\TalkBank\\CLAN\\CLAN.EXE only when that key does not exist
 * - Returns false, without probing any file, when the key exists but cannot
 *   be opened, or InstallPath is unreadable, not REG_SZ, empty, contains an
 *   embedded NUL, or is too long to hold "\\CLAN.EXE" within MAX_PATH
 * - Checks file attributes, not executable launchability
 * - launchCLANcApp()/send2clan() use the same admitted path and report
 *   SEND2CLAN_ERR_APP_NOT_FOUND for the same refusals. Verified against Win32
 *   test doubles; native Windows qualification remains separate.
 *
 * @section usage_example Example Usage
 *
 * @code{.c}
 * if (!is_clan_available()) {
 *     fprintf(stderr, "CLAN is not installed\n");
 *     fprintf(stderr, "Download from: https://dali.talkbank.org/clan/\n");
 *
 *     // Optionally provide graceful degradation
 *     // e.g., log to file instead of opening in CLAN
 *     return 1;
 * }
 *
 * // Availability is only a hint; sending has independent failure modes.
 * int result = send2clan(30, file, line, col, msg);
 * @endcode
 *
 * @section caching Caching Recommendations
 *
 * A cached answer can become stale. This illustrative cache requires caller
 * synchronization if shared between threads; it is not a delivery capability:
 *
 * @code{.c}
 * static bool clan_checked = false;
 * static bool clan_available = false;
 *
 * if (!clan_checked) {
 *     clan_available = is_clan_available();
 *     clan_checked = true;
 * }
 *
 * if (clan_available) {
 *     send2clan(30, file, line, col, msg);
 * }
 * @endcode
 *
 * @section thread_safety Thread Safety
 *
 * **Thread-safe**: Multiple threads can call this function concurrently.
 *
 * @section performance Performance
 *
 * No latency bound is established. Cache invalidation belongs to the caller.
 *
 * @return true if the platform location heuristic reports a match
 * @return false if CLAN not found or platform unsupported
 *
 * @see is_platform_supported()
 * @see send2clan_get_capabilities()
 */
SEND2CLAN_API bool is_clan_available(void);

#ifdef __cplusplus
}
#endif

#endif // SEND2CLAN_H
