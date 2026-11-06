// Tiến trình bộ gõ: giữ IMKServer chạy. `Songngu --register` chỉ đăng ký nguồn nhập với hệ thống rồi thoát
// (người dùng vẫn tự bật trong System Settings > Keyboard > Input Sources).
#import <Carbon/Carbon.h>
#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#include "songngu_mac.h"

static IMKServer *gServer;

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
        [[NSApplication sharedApplication] run];
    }
    return 0;
}
