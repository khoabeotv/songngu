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
void sn_set_blind(SnHandle *h, bool on);  // phím sắp tới đi đường chặn phím dù app không trong danh sách
void sn_set_tap_app(SnHandle *h, const char *bundle_id, bool on);  // người dùng chọn, lưu vào settings.tsv
void sn_set_tap_auto(SnHandle *h, const char *bundle_id);          // bộ gõ tự phát hiện, lưu kèm dấu tự ghi
void sn_reload(SnHandle *h);  // nạp lại settings.tsv/memory.tsv nếu cửa sổ cài đặt đã sửa
bool sn_save_memory(SnHandle *h);
uint32_t sn_memory_size(SnHandle *h);  // số từ đã học từ các lần lật
void sn_forget_memory(SnHandle *h);    // quên hết (file cũ đổi tên thành memory-forgotten.tsv)
bool sn_option_get(SnHandle *h, const char *name);
void sn_option_toggle(SnHandle *h, const char *name);

// Cửa sổ cài đặt (src/ffi_settings.rs): sửa thẳng file người dùng theo đường dẫn; chuỗi trả về giải phóng bằng sn_str_free.
void sn_str_free(char *s);
bool sn_cfg_option(const char *path, const char *name);
bool sn_cfg_toggle_option(const char *path, const char *name);
char *sn_cfg_list(const char *path, const char *list);  // "exclude" | "tap" | "tap_auto" | "builtin_tap", mỗi dòng một bundle id
bool sn_cfg_set(const char *path, const char *list, const char *bundle, bool on);  // "exclude" | "tap"
char *sn_mem_learned(const char *path);  // mỗi dòng: khoá \t chữ \t chọn \t bỏ \t ngữ cảnh
bool sn_mem_forget(const char *path, const char *key);
bool sn_mem_forget_all(const char *path);
char *sn_mem_habits(const char *path);  // mỗi dòng: late|late_d \t số từ có thể gõ muộn \t số từ gõ muộn
