// Dùng chung giữa SongnguController.m (InputMethodKit) và KeyTap.m (chặn phím).
#pragma once
#import <Cocoa/Cocoa.h>
#include "songngu.h"

// Đánh dấu sự kiện phím bộ gõ tự bắn (kCGEventSourceUserData), để không xử lý lại.
#define SN_SYNTHETIC 0x534E4755

void songnguLoad(void);
// File trong ~/Library/Application Support/Songngu.
NSString *songnguSupportFile(NSString *name);
SnHandle *songnguEngine(void);
// Đổi app đang gõ: kết thúc từ dở ở app cũ rồi áp cài đặt của app mới.
void songnguSetApp(NSString *bundle);
// Phím -> loại sự kiện của engine (SN_*); `ch` là ký tự khi SN_CHAR.
uint32_t songnguEventKind(NSEvent *event, uint32_t *ch);
// Lật (như Shift/Ctrl+Esc) ở ô nhập đang mở phiên của app `bundle`; NO nếu không có gì để lật (phím đi tiếp tới app).
BOOL songnguFlipFromTap(NSString *bundle);

// Đường chặn phím (cần quyền Trợ năng): app trong danh sách `sn_tap_app` nhận chữ bằng Backspace + phím Unicode giả.
void keyTapStart(void);
// Đường chặn phím đang lo app này: lớp InputMethodKit để phím đi qua.
BOOL keyTapOwns(NSString *bundle);
// App vừa mở (YES) / đóng (NO) phiên nhập liệu với bộ gõ. App đang không có phiên thì đường chặn phím tự lo.
void keyTapSession(NSString *bundle, BOOL open);
BOOL keyTapRunning(void);
// Xoá `erase` ký tự rồi gõ `text` bằng phím giả (khi app lỡ không báo vị trí con trỏ). NO nếu chưa có quyền Trợ năng.
BOOL keyTapPostEdit(uint32_t erase, NSString *text);
// Phím này do bộ gõ tự bắn: lớp IMK để app xử lý.
BOOL keyTapIsOurs(NSEvent *event);
// Mở hộp thoại xin quyền Trợ năng.
void keyTapAskPermission(void);
// App đang ở trước mặt (menu bộ gõ áp cho app này).
NSString *keyTapFrontApp(void);
