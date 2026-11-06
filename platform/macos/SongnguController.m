// Lớp InputMethodKit mỏng: đổi NSEvent thành sự kiện của engine rồi áp kết quả (chèn chữ, thay chữ trước con trỏ).
// Không có vùng soạn (gạch chân). Mọi quyết định nằm trong Rust (src/ime.rs). Thay chữ dùng
// insertText:replacementRange: của app; app không thay được thì đường chặn phím (KeyTap.m) lo thay, IMK để phím đi qua.
#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#include "songngu_mac.h"

static SnHandle *gEngine;
static NSString *gApp;

static NSString *supportFile(NSString *name) {
    return [NSHomeDirectory() stringByAppendingPathComponent:[@"Library/Application Support/Songngu" stringByAppendingPathComponent:name]];
}

// Nạp mô hình (mmap, vài ms). main.m gọi ngay khi tiến trình khởi động.
void songnguLoad(void) {
    if (gEngine) return;
    NSString *res = [[NSBundle mainBundle] resourcePath];
    NSString *data = [res stringByAppendingPathComponent:@"data"];
    NSString *model = [res stringByAppendingPathComponent:@"model.bin"];
    BOOL hasModel = [[NSFileManager defaultManager] fileExistsAtPath:model];
    gEngine = sn_create(data.UTF8String, hasModel ? model.UTF8String : NULL, supportFile(@"memory.tsv").UTF8String,
                        supportFile(@"settings.tsv").UTF8String, supportFile(@"typing-log.tsv").UTF8String);
    if (!gEngine) NSLog(@"Songngu: cannot load dictionaries from %@", data);
}

SnHandle *songnguEngine(void) {
    return gEngine;
}

void songnguSetApp(NSString *bundle) {
    if (!gEngine || !bundle || [bundle isEqualToString:gApp]) return;
    sn_commit_all(gEngine);
    sn_set_app(gEngine, bundle.UTF8String);
    gApp = bundle;
}

uint32_t songnguEventKind(NSEvent *event, uint32_t *ch) {
    NSEventModifierFlags flags = event.modifierFlags;
    if (event.keyCode == 53) {
        if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagOption)) return SN_OTHER;
        // Ctrl+Esc, Shift+Esc: lật ở mọi app (terminal/IDE: Esc trơn là của app)
        return (flags & (NSEventModifierFlagShift | NSEventModifierFlagControl)) ? SN_FLIP : SN_ESCAPE;
    }
    if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption)) return SN_OTHER;
    switch (event.keyCode) {
        case 51: return SN_BACKSPACE;
        case 36: case 76: case 48:                          // Return, Enter, Tab
        case 117: case 115: case 119: case 116: case 121:  // Fwd Delete, Home, End, PgUp, PgDn
        case 123: case 124: case 125: case 126:             // mũi tên
            return SN_OTHER;
    }
    NSString *chars = event.characters;
    if (chars.length != 1) return SN_OTHER;
    unichar c = [chars characterAtIndex:0];
    if (c < 0x20 || c == 0x7f || (c >= 0xF700 && c <= 0xF8FF)) return SN_OTHER;  // điều khiển, phím chức năng
    *ch = c;
    return SN_CHAR;
}

// Ghi vết sự kiện để gỡ lỗi theo app (có file `trace` trong thư mục Songngu lúc khởi động): trace.log ghi cả chữ gõ,
// xoá `trace` và `trace.log` khi xong.
static FILE *traceFile(void) {
    static FILE *f;
    static BOOL checked;
    if (!checked) {
        checked = YES;
        if ([[NSFileManager defaultManager] fileExistsAtPath:supportFile(@"trace")]) f = fopen(supportFile(@"trace.log").UTF8String, "a");
    }
    return f;
}

static void trace(id sender, NSString *what) {
    FILE *f = traceFile();
    if (!f) return;
    NSRange sel = [sender selectedRange];
    fprintf(f, "%.3f %s sel=%ld,%ld %s\n", CFAbsoluteTimeGetCurrent(), [[sender bundleIdentifier] UTF8String],
            (long)sel.location, (long)sel.length, what.UTF8String);
    fflush(f);
}

// Chữ ngay trước con trỏ (tối đa 64 ký tự); nil nếu app không cho đọc.
static NSString *textBefore(id sender) {
    NSRange sel = [sender selectedRange];
    if (sel.location == NSNotFound || sel.location == 0) return sel.location == 0 ? @"" : nil;
    NSUInteger n = MIN((NSUInteger)64, sel.location);
    NSAttributedString *s = [sender attributedSubstringFromRange:NSMakeRange(sel.location - n, n)];
    return s ? s.string : nil;
}

// App không báo vị trí con trỏ ngần này lần thay liền thì chuyển hẳn sang chặn phím. Một lần lỡ nhịp (Flutter, Electron báo vị
// trí trễ ở đầu ô nhập) không đủ để kết luận cả app.
static const int kNoCursorLimit = 3;

static void apply(SnAction a, id sender) {
    static int noCursor;
    static NSString *noCursorApp;
    NSString *commit = [NSString stringWithUTF8String:a.commit];
    NSRange replace = NSMakeRange(NSNotFound, NSNotFound);
    if (a.replace_before > 0) {
        NSRange sel = [sender selectedRange];
        NSString *bundle = [sender bundleIdentifier];
        if (sel.location != NSNotFound && sel.location >= a.replace_before) {
            replace = NSMakeRange(sel.location - a.replace_before, a.replace_before);
            noCursor = 0;
        } else {
            noCursor = [bundle isEqualToString:noCursorApp] ? noCursor + 1 : 1;
            noCursorApp = bundle;
            if (noCursor >= kNoCursorLimit && bundle && !sn_tap_app(gEngine, bundle.UTF8String))
                sn_set_tap_app(gEngine, bundle.UTF8String, true);
            // lần này: sửa bằng phím giả nếu có quyền, không thì thử ký tự DEL (nhiều app bỏ qua)
            if (keyTapPostEdit(a.erase, commit)) return;
            commit = [[@"" stringByPaddingToLength:a.erase withString:@"\x7f" startingAtIndex:0] stringByAppendingString:commit];
        }
    }
    // commit rỗng + có vùng thay = xoá (Backspace làm từ ngắn hơn một chữ)
    if (commit.length || replace.location != NSNotFound) [sender insertText:commit replacementRange:replace];
}

@interface SongnguController : IMKInputController
@end

@implementation SongnguController

- (NSUInteger)recognizedEvents:(id)sender {
    return NSEventMaskKeyDown;
}

- (void)activateServer:(id)sender {
    [super activateServer:sender];
    trace(sender, @"activate");
    songnguLoad();
    songnguSetApp([sender bundleIdentifier]);
}

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
    if (event.type != NSEventTypeKeyDown) return NO;
    songnguLoad();
    if (!gEngine) return NO;
    // phím bộ gõ tự bắn, hoặc app do đường chặn phím lo: để app tự xử lý
    if (keyTapIsOurs(event)) return NO;
    NSString *bundle = [sender bundleIdentifier];
    if (keyTapOwns(bundle)) return NO;
    static BOOL asked;
    if (!asked && !keyTapRunning() && bundle && sn_tap_app(gEngine, bundle.UTF8String)) {
        // app cần chặn phím (terminal) mà chưa có quyền: hỏi một lần mỗi lần bộ gõ chạy
        asked = YES;
        keyTapAskPermission();
    }
    songnguSetApp(bundle);
    uint32_t ch = 0;
    uint32_t kind = songnguEventKind(event, &ch);
    // Chỉ đọc chữ trong app khi engine cần: đầu từ, đang gõ, Esc.
    BOOL wantsBefore = kind == SN_ESCAPE || kind == SN_FLIP || sn_wants_before(gEngine);
    NSString *before = wantsBefore ? textBefore(sender) : nil;
    SnAction a = sn_event(gEngine, kind, ch, before.UTF8String);
    if (traceFile())
        trace(sender, [NSString stringWithFormat:@"key kind=%u ch=%C before=[%@] -> commit=[%@] replace=%u erase=%u pass=%d tap=%d", kind,
                                                 (unichar)(ch ?: '-'), before ?: @"(nil)", @(a.commit), a.replace_before, a.erase,
                                                 a.pass, keyTapRunning()]);
    apply(a, sender);
    return !a.pass;
}

// Hệ thống gọi khi click chuột, đổi ô nhập, đổi app: chữ đã nằm trong app, chỉ kết thúc từ đang gõ.
- (void)commitComposition:(id)sender {
    trace(sender, @"commitComposition");
    if (gEngine && !keyTapOwns([sender bundleIdentifier])) sn_commit_all(gEngine);
}

- (void)deactivateServer:(id)sender {
    trace(sender, @"deactivate");
    [self commitComposition:sender];
    if (gEngine) sn_save_memory(gEngine);
    [super deactivateServer:sender];
}

// Menu của bộ gõ trên thanh menu. Mục theo app áp cho app trước mặt.
- (NSMenu *)menu {
    NSMenu *m = [[NSMenu alloc] initWithTitle:@"Songngu"];
    NSString *front = keyTapFrontApp();
    NSMenuItem *tap = [[NSMenuItem alloc] initWithTitle:@"Chặn phím ở app này (terminal, app không nhận bộ gõ)"
                                                 action:@selector(toggleTap:)
                                          keyEquivalent:@""];
    tap.representedObject = front;
    tap.state = (gEngine && front && sn_tap_app(gEngine, front.UTF8String)) ? NSControlStateValueOn : NSControlStateValueOff;
    NSArray *items = @[
        @[ @"Bỏ qua app này", @"exclude" ], tap, @[ @"Đặt dấu kiểu cũ (hòa, thủy)", @"classic_tone" ], @[ @"Gõ VNI", @"vni" ],
        @[ @"[ ] gõ ơ ư", @"brackets" ], @[ @"Ghi nhật ký gõ (chỉ trên máy này)", @"log" ]
    ];
    for (id it in items) {
        if ([it isKindOfClass:[NSMenuItem class]]) {
            [m addItem:it];
            continue;
        }
        NSMenuItem *mi = [[NSMenuItem alloc] initWithTitle:it[0] action:@selector(toggleOption:) keyEquivalent:@""];
        mi.representedObject = it[1];
        mi.state = (gEngine && sn_option_get(gEngine, [it[1] UTF8String])) ? NSControlStateValueOn : NSControlStateValueOff;
        [m addItem:mi];
    }
    if (!keyTapRunning()) {
        [m addItem:[NSMenuItem separatorItem]];
        [m addItem:[[NSMenuItem alloc] initWithTitle:@"Cấp quyền Trợ năng (để gõ trong terminal)…"
                                              action:@selector(askPermission:)
                                       keyEquivalent:@""]];
    }
    // Không hỏi lại (hộp thoại từ bộ gõ sẽ giành focus của app): file cũ vẫn giữ ở memory-forgotten.tsv.
    [m addItem:[NSMenuItem separatorItem]];
    uint32_t learned = gEngine ? sn_memory_size(gEngine) : 0;
    NSString *title = [NSString stringWithFormat:@"Quên những gì đã học (%u từ)", learned];
    NSMenuItem *forget = [[NSMenuItem alloc] initWithTitle:title action:@selector(forgetMemory:) keyEquivalent:@""];
    forget.enabled = learned > 0;
    [m addItem:forget];
    return m;
}

static NSMenuItem *menuItem(id sender) {
    return [sender isKindOfClass:[NSDictionary class]] ? sender[kIMKCommandMenuItemName] : sender;
}

- (void)forgetMemory:(id)sender {
    if (gEngine) sn_forget_memory(gEngine);
}

- (void)askPermission:(id)sender {
    keyTapAskPermission();
}

- (void)toggleTap:(id)sender {
    NSString *app = menuItem(sender).representedObject;
    if (!gEngine || !app) return;
    sn_set_tap_app(gEngine, app.UTF8String, !sn_tap_app(gEngine, app.UTF8String));
}

- (void)toggleOption:(id)sender {
    NSString *key = menuItem(sender).representedObject;
    if (gEngine && key) sn_option_toggle(gEngine, key.UTF8String);
}

@end
