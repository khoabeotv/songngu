// C ABI của engine (src/ffi.rs).
#pragma once
#include <stdbool.h>
#include <stdint.h>

typedef struct SnHandle SnHandle;

// Xoá phần ngay trước con trỏ rồi chèn `commit`.
typedef struct {
    const char *commit;       // chữ chèn vào app
    uint32_t replace_before;  // phần bị xoá, tính theo đơn vị UTF-16 (API thay chữ)
    uint32_t erase;           // phần bị xoá, tính theo ký tự (số lần Backspace ở đường chặn phím)
    bool pass;                // để app xử lý phím (sau khi đã áp)
} SnAction;

enum { SN_CHAR = 0, SN_BACKSPACE = 1, SN_ESCAPE = 2, SN_OTHER = 3, SN_FLIP = 4 };

SnHandle *sn_create(const char *data_dir, const char *model, const char *memory, const char *settings, const char *log);
void sn_set_app(SnHandle *h, const char *bundle_id);
SnAction sn_event(SnHandle *h, uint32_t kind, uint32_t ch, const char *before);
bool sn_wants_before(SnHandle *h);  // phím sắp tới cần chữ trước con trỏ (Esc luôn cần, trừ đường chặn phím)
void sn_commit_all(SnHandle *h);    // kết thúc từ đang gõ (click, đổi app); chữ đã nằm trong app
bool sn_tap_app(SnHandle *h, const char *bundle_id);              // app dùng đường chặn phím + Backspace
void sn_set_tap_app(SnHandle *h, const char *bundle_id, bool on);  // lưu vào settings.tsv
bool sn_save_memory(SnHandle *h);
uint32_t sn_memory_size(SnHandle *h);  // số từ đã học từ các lần lật
void sn_forget_memory(SnHandle *h);    // quên hết (file cũ đổi tên thành memory-forgotten.tsv)
bool sn_option_get(SnHandle *h, const char *name);
void sn_option_toggle(SnHandle *h, const char *name);
