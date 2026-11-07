// Tiến trình bộ gõ: giữ IMKServer chạy. `Songngu --register` chỉ đăng ký nguồn nhập với hệ thống rồi thoát
// (người dùng vẫn tự bật trong System Settings > Keyboard > Input Sources).
#import <Carbon/Carbon.h>
#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#include "songngu_mac.h"

static IMKServer *gServer;

// Tự cập nhật (tuỳ chọn auto_update): bộ gõ không truy cập mạng, chỉ gọi cửa sổ cài đặt chạy ngầm; cửa sổ tự giới hạn
// mỗi ngày một lần và tự thay Songngu.app (settings/Updater.swift).
static void checkUpdate(void) {
    SnHandle *h = songnguEngine();
    if (!h) return;
    sn_reload(h);
    if (!sn_option_get(h, "auto_update")) return;
    NSURL *exe = [NSBundle.mainBundle.bundleURL
        URLByAppendingPathComponent:@"Contents/Helpers/SongnguSettings.app/Contents/MacOS/SongnguSettings"];
    [NSTask launchedTaskWithExecutableURL:exe arguments:@[ @"--update" ] error:nil terminationHandler:nil];
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSBundle *bundle = [NSBundle mainBundle];
        if (argc > 1 && strcmp(argv[1], "--register") == 0) {
            OSStatus st = TISRegisterInputSource((__bridge CFURLRef)bundle.bundleURL);
            printf("TISRegisterInputSource: %d\n", (int)st);
            if (st != noErr) return 1;
            // macOS không cho tiến trình khác bật/chọn bộ gõ bên thứ ba: người dùng tự thêm.
            NSDictionary *filter = @{(__bridge NSString *)kTISPropertyInputSourceID : bundle.bundleIdentifier};
            NSArray *srcs = CFBridgingRelease(TISCreateInputSourceList((__bridge CFDictionaryRef)filter, false));
            if (!srcs.count) printf("Chưa bật: System Settings > Keyboard > Text Input > Edit > + > Vietnamese > Songngu\n");
            return 0;
        }
        NSString *conn = [bundle objectForInfoDictionaryKey:@"InputMethodConnectionName"];
        gServer = [[IMKServer alloc] initWithName:conn bundleIdentifier:bundle.bundleIdentifier];
        songnguLoad();
        keyTapStart();
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 120 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{ checkUpdate(); });
        [NSTimer scheduledTimerWithTimeInterval:3600 repeats:YES block:^(NSTimer *t) { checkUpdate(); }];
        [[NSApplication sharedApplication] run];
    }
    return 0;
}
