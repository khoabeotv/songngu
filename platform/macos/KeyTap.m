// Đường chặn phím: cho app mà InputMethodKit không thay được chữ (terminal: chữ đã gửi vào shell, vị trí con trỏ
// luôn 0; app bỏ qua lệnh thay chữ; app không đưa phím qua bộ gõ). Chặn phím ở cấp phiên (CGEventTap, cần quyền Trợ năng),
// đưa qua engine như đường IMK nhưng không đọc chữ trong app (`blind`), rồi áp bằng Backspace giả + phím mang
// chuỗi Unicode. Phím không đổi gì thì để nguyên phím thật đi qua. Chỉ chạy khi Songngu là nguồn nhập đang chọn.
#import <Carbon/Carbon.h>
#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include "songngu_mac.h"

static CFMachPortRef gTap;
static CGEventSourceRef gSource;
static BOOL gSelected;              // Songngu là nguồn nhập đang chọn
static NSString *gFront;            // app trước mặt
static NSMutableDictionary<NSNumber *, NSString *> *gPidBundle;
static NSMutableIndexSet *gSwallowed;  // phím đã nuốt keyDown: nuốt luôn keyUp
static BOOL gTyping;                // phím gần nhất đi đường chặn phím (click chuột thì kết thúc từ)
static NSString *gSession;          // app đang mở phiên nhập liệu với bộ gõ (activateServer)

BOOL keyTapRunning(void) {
    return gTap && CGEventTapIsEnabled(gTap);
}

NSString *keyTapFrontApp(void) {
    return gFront ?: NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier;
}

void keyTapSession(NSString *bundle, BOOL open) {
    if (open) gSession = bundle;
    else if ([bundle isEqualToString:gSession]) gSession = nil;
}

BOOL keyTapOwns(NSString *bundle) {
    SnHandle *h = songnguEngine();
    return h && bundle && keyTapRunning() && gSelected && sn_tap_app(h, bundle.UTF8String);
}

static void updateSelected(void) {
    TISInputSourceRef s = TISCopyCurrentKeyboardInputSource();
    NSString *sid = (__bridge NSString *)TISGetInputSourceProperty(s, kTISPropertyInputSourceID);
    gSelected = [sid hasPrefix:NSBundle.mainBundle.bundleIdentifier];
    CFRelease(s);
}

// App nhận phím: tiến trình đích của sự kiện (bảng nổi như Raycast không phải app trước mặt), không có thì app trước mặt.
static NSString *targetBundle(CGEventRef ev) {
    pid_t pid = (pid_t)CGEventGetIntegerValueField(ev, kCGEventTargetUnixProcessID);
    if (pid > 0 && pid != getpid()) {
        NSNumber *k = @(pid);
        NSString *b = gPidBundle[k];
        if (!b) {
            b = [NSRunningApplication runningApplicationWithProcessIdentifier:pid].bundleIdentifier ?: @"";
            if (gPidBundle.count > 256) [gPidBundle removeAllObjects];
            gPidBundle[k] = b;
        }
        if (b.length) return b;
    }
    return keyTapFrontApp();
}

// `proxy` NULL: gửi vào phiên (ngoài callback của tap), xếp sau phím đang xử lý.
static void post(CGEventTapProxy proxy, CGKeyCode code, const UniChar *chars, UniCharCount n) {
    for (int down = 1; down >= 0; down--) {
        CGEventRef e = CGEventCreateKeyboardEvent(gSource, code, down);
        CGEventSetFlags(e, 0);
        if (n) CGEventKeyboardSetUnicodeString(e, n, chars);
        CGEventSetIntegerValueField(e, kCGEventSourceUserData, SN_SYNTHETIC);
        if (proxy) CGEventTapPostEvent(proxy, e);
        else CGEventPost(kCGSessionEventTap, e);
        CFRelease(e);
    }
}

// Xoá `erase` ký tự rồi gõ `text`, mỗi ký tự một phím (app đọc từng phím như Flutter cũng nhận đúng).
static void apply(CGEventTapProxy proxy, uint32_t erase, NSString *text) {
    for (uint32_t i = 0; i < erase; i++) post(proxy, kVK_Delete, NULL, 0);
    [text enumerateSubstringsInRange:NSMakeRange(0, text.length)
                             options:NSStringEnumerationByComposedCharacterSequences
                          usingBlock:^(NSString *s, NSRange r, NSRange er, BOOL *stop) {
                              UniChar buf[8];
                              UniCharCount n = MIN(s.length, (NSUInteger)8);
                              [s getCharacters:buf range:NSMakeRange(0, n)];
                              post(proxy, kVK_ANSI_A, buf, n);
                          }];
}

// Phím giả gửi qua keyTapPostEdit còn chưa tới lớp IMK (phòng khi app làm mất dấu SN_SYNTHETIC trên đường đi).
static int gPendingIMK;

BOOL keyTapPostEdit(uint32_t erase, NSString *text) {
    if (!gSource || !AXIsProcessTrusted()) return NO;
    __block int n = (int)erase;
    [text enumerateSubstringsInRange:NSMakeRange(0, text.length)
                             options:NSStringEnumerationByComposedCharacterSequences
                          usingBlock:^(NSString *s, NSRange r, NSRange er, BOOL *stop) { n++; }];
    gPendingIMK += n;
    apply(NULL, erase, text);
    return YES;
}

BOOL keyTapIsOurs(NSEvent *event) {
    BOOL marked = event.CGEvent && CGEventGetIntegerValueField(event.CGEvent, kCGEventSourceUserData) == SN_SYNTHETIC;
    BOOL pending = gPendingIMK > 0 && (event.keyCode == kVK_Delete || event.keyCode == kVK_ANSI_A);
    if (pending) gPendingIMK--;
    return marked || pending;
}

static CGEventRef onEvent(CGEventTapProxy proxy, CGEventType type, CGEventRef ev, void *ctx) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        CGEventTapEnable(gTap, true);
        return ev;
    }
    if (CGEventGetIntegerValueField(ev, kCGEventSourceUserData) == SN_SYNTHETIC) return ev;
    SnHandle *h = songnguEngine();
    if (!h || !gSelected) return ev;
    if (type == kCGEventLeftMouseDown || type == kCGEventRightMouseDown || type == kCGEventOtherMouseDown) {
        // click có thể dời con trỏ: không sửa chữ trước nó nữa
        if (gTyping) sn_commit_all(h);
        gTyping = NO;
        return ev;
    }
    CGKeyCode code = (CGKeyCode)CGEventGetIntegerValueField(ev, kCGKeyboardEventKeycode);
    if (type == kCGEventKeyUp) {
        if (![gSwallowed containsIndex:code]) return ev;
        [gSwallowed removeIndex:code];
        return NULL;
    }
    if (type != kCGEventKeyDown) return ev;
    NSString *bundle = targetBundle(ev);
    BOOL listed = sn_tap_app(h, bundle.UTF8String);
    // Shift/Ctrl+Esc ở app đi đường InputMethodKit: vài app tự giữ Esc có phím bổ trợ, không đưa qua bộ gõ (TablePlus),
    // nên lật ngay tại đây qua ô nhập đang mở phiên. Không có gì để lật thì phím tới app như thường.
    CGEventFlags flags = CGEventGetFlags(ev);
    if (!listed && code == kVK_Escape && [bundle isEqualToString:gSession] &&
        (flags & (kCGEventFlagMaskShift | kCGEventFlagMaskControl)) &&
        !(flags & (kCGEventFlagMaskCommand | kCGEventFlagMaskAlternate))) {
        if (!songnguFlipFromTap(bundle)) return ev;
        [gSwallowed addIndex:code];
        return NULL;
    }
    // App không mở phiên nhập liệu (terminal kiểu gpui, Flutter ở vài máy): phím không bao giờ tới bộ gõ, đường này lo.
    // Bỏ qua phím giữ lặp (game, phím di chuyển). Phím cho đi qua thì đánh dấu, phòng phiên mở muộn hơn phím.
    // Cửa sổ cài đặt (không có ô nhập chữ) cũng không mở phiên: bỏ qua, phím của nó là phím tắt.
    BOOL orphan = !listed && ![bundle isEqualToString:gSession] && ![bundle hasPrefix:@"com.songngu."] &&
                  !sn_option_get(h, "manual_tap") &&
                  !CGEventGetIntegerValueField(ev, kCGKeyboardEventAutorepeat);
    if (!listed && !orphan) {
        gTyping = NO;
        return ev;
    }
    songnguSetApp(bundle);
    sn_set_blind(h, true);
    gTyping = YES;
    if (orphan) CGEventSetIntegerValueField(ev, kCGEventSourceUserData, SN_SYNTHETIC);
    NSEvent *e = [NSEvent eventWithCGEvent:ev];
    uint32_t ch = 0;
    uint32_t kind = songnguEventKind(e, &ch);
    SnAction a = sn_event(h, kind, ch, NULL);
    NSString *text = [NSString stringWithUTF8String:a.commit];
    if (a.pass) {
        if (a.erase == 0 && !text.length) return ev;
        // sửa chữ trước rồi mới tới phím của app: bắn lại chính phím đó sau phần sửa
        apply(proxy, a.erase, text);
        CGEventRef copy = CGEventCreateCopy(ev);
        CGEventSetIntegerValueField(copy, kCGEventSourceUserData, SN_SYNTHETIC);
        CGEventTapPostEvent(proxy, copy);
        CFRelease(copy);
        return NULL;
    }
    // chỉ thêm đúng ký tự của phím: để phím thật đi qua
    if (a.erase == 0 && [text isEqualToString:e.characters]) return ev;
    apply(proxy, a.erase, text);
    [gSwallowed addIndex:code];
    return NULL;
}

static BOOL createTap(void) {
    CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp) | CGEventMaskBit(kCGEventLeftMouseDown) |
                       CGEventMaskBit(kCGEventRightMouseDown) | CGEventMaskBit(kCGEventOtherMouseDown);
    gTap = CGEventTapCreate(kCGAnnotatedSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, onEvent, NULL);
    if (!gTap) return NO;
    CFRunLoopSourceRef src = CFMachPortCreateRunLoopSource(NULL, gTap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), src, kCFRunLoopCommonModes);
    CFRelease(src);
    CGEventTapEnable(gTap, true);
    return YES;
}

// status.tsv cho cửa sổ cài đặt (tiến trình khác, không tự hỏi được quyền của bộ gõ): ghi lại khi trạng thái đổi.
static void writeStatus(void) {
    static NSString *last;
    NSString *s = [NSString stringWithFormat:@"pid\t%d\naccessibility\t%d\nkeytap\t%d\n", getpid(), AXIsProcessTrusted(),
                                             keyTapRunning()];
    if ([s isEqualToString:last]) return;
    NSString *path = songnguSupportFile(@"status.tsv");
    [[NSFileManager defaultManager] createDirectoryAtPath:path.stringByDeletingLastPathComponent
                              withIntermediateDirectories:YES
                                               attributes:nil
                                                    error:nil];
    if ([s writeToFile:path atomically:YES encoding:NSUTF8StringEncoding error:nil]) last = s;
}

void keyTapAskPermission(void) {
    NSDictionary *opts = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt : @YES};
    AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)opts);
}

void keyTapStart(void) {
    gSource = CGEventSourceCreate(kCGEventSourceStatePrivate);
    gPidBundle = [NSMutableDictionary dictionary];
    gSwallowed = [NSMutableIndexSet indexSet];
    updateSelected();
    [[NSDistributedNotificationCenter defaultCenter] addObserverForName:(__bridge NSString *)kTISNotifySelectedKeyboardInputSourceChanged
                                                                 object:nil
                                                                  queue:NSOperationQueue.mainQueue
                                                             usingBlock:^(NSNotification *n) {
                                                                 updateSelected();
                                                                 SnHandle *h = songnguEngine();
                                                                 if (h) sn_commit_all(h);
                                                             }];
    gFront = NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier;
    [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidActivateApplicationNotification
                                                                object:nil
                                                                 queue:NSOperationQueue.mainQueue
                                                            usingBlock:^(NSNotification *n) {
                                                                NSRunningApplication *app = n.userInfo[NSWorkspaceApplicationKey];
                                                                gFront = app.bundleIdentifier;
                                                                gTyping = NO;
                                                            }];
    createTap();
    writeStatus();
    // chưa có quyền Trợ năng: thử lại tới khi người dùng cấp (không cần khởi động lại bộ gõ); quyền có thể bị gỡ bất cứ lúc
    // nào nên trạng thái cho cửa sổ cài đặt luôn được cập nhật
    [NSTimer scheduledTimerWithTimeInterval:3
                                    repeats:YES
                                      block:^(NSTimer *t) {
                                          if (!gTap && AXIsProcessTrusted()) createTap();
                                          writeStatus();
                                      }];
}
