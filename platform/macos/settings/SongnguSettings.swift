// Cửa sổ cài đặt Songngu (SwiftUI): app riêng trong Songngu.app/Contents/Helpers, menu SN mở nó. Chỉ sửa file người
// dùng; bộ gõ tự nạp lại khi file đổi, nên cửa sổ có lỗi hay đóng thì bộ gõ vẫn chạy.
import AppKit
import SwiftUI

@main
struct SongnguSettingsApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var store = Store()

    var body: some Scene {
        Window("Cài đặt Songngu", id: "main") {
            RootView(page: Capture.request?.page ?? .typing)
                .environmentObject(store)
                .frame(minWidth: 640, minHeight: 440)
        }
        .defaultSize(width: 760, height: 560)
        .windowResizability(.contentMinSize)
        .commands { CommandGroup(replacing: .newItem) {} }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    func applicationWillFinishLaunching(_ notification: Notification) {
        if Capture.request != nil { Capture.pretendActive() }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        Capture.start()
    }
}

struct SidebarIcon: View {
    let page: Page

    var body: some View {
        Image(systemName: page.symbol)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: 20, height: 20)
            .background(page.tint.gradient, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
    }
}

struct RootView: View {
    @EnvironmentObject var store: Store
    @State var page: Page?

    init(page: Page) {
        _page = State(initialValue: page)
    }

    var body: some View {
        NavigationSplitView {
            List(Page.allCases, selection: $page) { p in
                Label { Text(p.title) } icon: { SidebarIcon(page: p) }
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 260)
        } detail: {
            let current = page ?? .typing
            VStack(spacing: 0) {
                if let e = store.error {
                    Label(e, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 20)
                        .padding(.top, 10)
                }
                switch current {
                case .typing: TypingPage()
                case .apps: AppsPage()
                case .learned: LearnedPage()
                case .status: StatusPage()
                }
            }
            .navigationTitle(current.title)
        }
    }
}
