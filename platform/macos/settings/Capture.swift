// Tự chụp cửa sổ ra PNG để xem giao diện khi không ngồi trước máy (kể cả lúc máy khoá màn hình):
//   SONGNGU_SCREENSHOT=thư mục SONGNGU_PAGE=0..3 SONGNGU_APPEARANCE=light|dark SongnguSettings
// Chụp cửa sổ của chính tiến trình không cần quyền Ghi màn hình. Mỗi lần chạy một trang: cửa sổ bị màn hình khoá che
// thì không vẽ lại khi đổi trang.
import AppKit

enum Capture {
    struct Request {
        let dir: URL
        let page: Page
        let dark: Bool
    }

    static let request: Request? = {
        let env = ProcessInfo.processInfo.environment
        guard let dir = env["SONGNGU_SCREENSHOT"] else { return nil }
        let i = Int(env["SONGNGU_PAGE"] ?? "") ?? 0
        let page = Page.allCases[max(0, min(i, Page.allCases.count - 1))]
        return Request(dir: URL(fileURLWithPath: dir), page: page, dark: env["SONGNGU_APPEARANCE"] == "dark")
    }()

    /// Máy khoá màn hình thì app không được kích hoạt, cửa sổ vẽ kiểu không focus (control, sidebar mờ). Khi chụp, cho
    /// AppKit tin app và cửa sổ đang được focus.
    static func pretendActive() {
        func always(_ cls: AnyClass, _ sel: Selector) {
            guard let m = class_getInstanceMethod(cls, sel) else { return }
            let yes: @convention(block) (AnyObject) -> Bool = { _ in true }
            method_setImplementation(m, imp_implementationWithBlock(yes))
        }
        always(NSWindow.self, #selector(getter: NSWindow.isKeyWindow))
        always(NSWindow.self, #selector(getter: NSWindow.isMainWindow))
        always(NSApplication.self, #selector(getter: NSApplication.isActive))
        // AppKit chọn kiểu vẽ "đang focus" qua các hàm riêng này
        for name in ["hasKeyAppearance", "hasMainAppearance", "_hasActiveAppearance", "_hasActiveAppearanceIgnoringKeyFocus",
                     "_hasKeyAppearance", "_hasMainAppearance"] {
            always(NSWindow.self, NSSelectorFromString(name))
        }
    }

    @MainActor
    static func start() {
        guard let r = request else { return }
        NSApp.appearance = NSAppearance(named: r.dark ? .darkAqua : .aqua)
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            let index = Page.allCases.firstIndex(of: r.page) ?? 0
            let name = "\(index)-\(r.page.title)-\(r.dark ? "toi" : "sang").png"
            let out = r.dir.appendingPathComponent(name)
            if let w = NSApp.windows.first(where: { $0.isVisible }), let png = shoot(windowNumber: w.windowNumber) {
                try? FileManager.default.createDirectory(at: r.dir, withIntermediateDirectories: true)
                try? png.write(to: out)
                print("chụp \(out.path)")
            } else {
                print("lỗi: không chụp được \(name)")
            }
            NSApp.terminate(nil)
        }
    }

    /// CGWindowListCreateImage bị bỏ khỏi SDK từ macOS 15 nhưng vẫn có lúc chạy; lấy qua dlsym.
    private static func shoot(windowNumber: Int) -> Data? {
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage") else { return nil }
        let create = unsafeBitCast(sym, to: Create.self)
        // kCGWindowListOptionIncludingWindow, kCGWindowImageBoundsIgnoreFraming
        guard let img = create(.null, 1 << 3, UInt32(windowNumber), 1 << 0)?.takeRetainedValue() else { return nil }
        return NSBitmapImageRep(cgImage: img).representation(using: .png, properties: [:])
    }
}
