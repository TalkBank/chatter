/*
 * Deterministic Win32 test doubles for the send2clan bridge.
 *
 * This header stands in for <windows.h> when harness.c compiles the real
 * csrc/send2clan.c on a non-Windows host. It declares only the types,
 * constants and functions the bridge uses; harness.c defines the functions
 * with scripted behavior and call recording. Widths follow the Win32 ABI
 * (DWORD and LONG are 32-bit). This is a contract test of the bridge's own
 * logic, not a model of Windows: it cannot qualify native behavior.
 */
#ifndef SEND2CLAN_WIN32_DOUBLES_WINDOWS_H
#define SEND2CLAN_WIN32_DOUBLES_WINDOWS_H

#include <stdint.h>
#include <stdio.h>

typedef uint32_t DWORD;
typedef int32_t LONG;
typedef unsigned int UINT;
typedef unsigned char *LPBYTE;
typedef const char *LPCSTR;
typedef uintptr_t WPARAM;
typedef uintptr_t DWORD_PTR;
typedef intptr_t LPARAM;
typedef intptr_t LRESULT;
typedef DWORD REGSAM;

typedef struct win32_double_window *HWND;
typedef struct win32_double_key *HKEY;
typedef struct win32_double_instance *HINSTANCE;

#define MAX_PATH 260

#define ERROR_SUCCESS 0L
#define ERROR_FILE_NOT_FOUND 2L
#define ERROR_ACCESS_DENIED 5L
#define ERROR_MORE_DATA 234L
#define ERROR_TIMEOUT 1460L
#define SMTO_BLOCK 1u
#define SMTO_ABORTIFHUNG 2u
#define SMTO_ERRORONEXIT 0x20u

#define REG_SZ 1u
#define REG_EXPAND_SZ 2u
#define REG_BINARY 3u

#define KEY_READ 0x20019u
#define HKEY_LOCAL_MACHINE ((HKEY)(uintptr_t)0x80000002u)

#define INVALID_FILE_ATTRIBUTES ((DWORD)0xFFFFFFFFu)
#define SW_SHOWNORMAL 1
#define WM_APP 0x8000u

HWND FindWindowA(LPCSTR class_name, LPCSTR window_name);
LONG RegOpenKeyExA(HKEY key, LPCSTR sub_key, DWORD options, REGSAM desired, HKEY *result);
LONG RegQueryValueExA(HKEY key, LPCSTR value_name, DWORD *reserved, DWORD *type,
                      LPBYTE data, DWORD *data_size);
LONG RegCloseKey(HKEY key);
DWORD GetFileAttributesA(LPCSTR path);
HINSTANCE ShellExecuteA(HWND window, LPCSTR operation, LPCSTR file, LPCSTR parameters,
                        LPCSTR directory, int show);
void Sleep(DWORD milliseconds);
DWORD GetTickCount(void);
DWORD GetEnvironmentVariableA(LPCSTR name, char *buffer, DWORD size);
LRESULT SendMessageA(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
LRESULT SendMessageTimeoutA(HWND window, UINT message, WPARAM wparam, LPARAM lparam, UINT flags, UINT timeout, DWORD_PTR *reply);
void SetLastError(DWORD error);
DWORD GetLastError(void);
int fopen_s(FILE **stream, const char *path, const char *mode);

/* MSVC CRT bounded-string functions. The current bridge no longer calls them;
 * they exist so the negative control can compile the pre-repair bridge. */
int strcpy_s(char *destination, size_t size, const char *source);
int strcat_s(char *destination, size_t size, const char *source);

#endif
