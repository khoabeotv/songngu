// Dữ liệu của cửa sổ cài đặt: đọc/sửa file người dùng qua C ABI của thư viện Rust (src/ffi_settings.rs), để cách đọc
// settings.tsv/memory.tsv chỉ nằm một chỗ. Bộ gõ là tiến trình khác, tự nạp lại khi file đổi; status.tsv do bộ gõ ghi.
import AppKit
import SwiftUI
import Carbon
import UniformTypeIdentifiers
import UserNotifications

enum Page: String, CaseIterable, Identifiable {
    case start, typing, apps, learned, status, report
    var id: Self { self }

    /// `SongnguSettings --page report`: menu SN mở thẳng một trang.
    static var fromArguments: Page? {
        let args = CommandLine.arguments
        guard let i = args.firstIndex(of: "--page"), i + 1 < args.count else { return nil }
        return Page(rawValue: args[i + 1])
    }

    var title: String {
        switch self {
        case .start: return "Bắt đầu"
        case .typing: return "Gõ"
        case .apps: return "Ứng dụng"
        case .learned: return "Đã học"
        case .status: return "Trạng thái"
        case .report: return "Báo lỗi"
        }
    }

    var symbol: String {
        switch self {
        case .start: return "checklist"
        case .typing: return "keyboard"
        case .apps: return "square.grid.2x2.fill"
        case .learned: return "brain.head.profile"
        case .status: return "checkmark.shield.fill"
        case .report: return "ladybug.fill"
        }
    }

    var tint: Color {
        switch self {
        case .start: return .orange
        case .typing: return .gray
        case .apps: return .blue
        case .learned: return .pink
        case .status: return .green
        case .report: return .red
        }
    }
}

struct AppEntry: Identifiable {
    let bundle: String
    let name: String
    let icon: NSImage
    var id: String { bundle }

    static func of(_ bundle: String) -> AppEntry {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundle) {
            let name = FileManager.default.displayName(atPath: url.path).replacingOccurrences(of: ".app", with: "")
            return AppEntry(bundle: bundle, name: name, icon: NSWorkspace.shared.icon(forFile: url.path))
        }
        return AppEntry(bundle: bundle, name: bundle, icon: NSWorkspace.shared.icon(for: .application))
    }
}

/// Thói quen gõ dấu muộn bộ gõ đã học (memory.tsv): số từ có thể gõ muộn, số từ gõ muộn.
struct Habit: Identifiable {
    let name: String
    let total: Double
    let late: Double
    var id: String { name }

    var title: String { name == "late" ? "Gõ mũ muộn (cana → cân, nene → nên)" : "Gõ đ muộn (did → đi, duod → đươ)" }
}

struct LearnedWord: Identifiable {
    let key: String
    let word: String
    let detail: String
    var id: String { key }
}

/// Bộ gõ chạy nền bắt phím (conflicts.tsv trong Resources): chạy cùng Songngu thì cả hai cùng sửa phím.
struct Conflict: Identifiable {
    let bundle: String
    let name: String
    let process: String
    var id: String { bundle }

    static func load(_ resources: URL?) -> [Conflict] {
        guard let r = resources,
              let text = try? String(contentsOf: r.appendingPathComponent("conflicts.tsv"), encoding: .utf8) else { return [] }
        return text.split(separator: "\n").compactMap { line in
            guard !line.hasPrefix("#") else { return nil }
            let f = line.split(separator: "\t").map(String.init)
            return f.count == 3 ? Conflict(bundle: f[0], name: f[1], process: f[2]) : nil
        }
    }

    /// Các bộ gõ trong danh sách đang chạy (theo bundle id, hoặc tên tiến trình nếu bản khác đổi bundle id).
    static func running(_ known: [Conflict]) -> [Conflict] {
        let apps = NSWorkspace.shared.runningApplications
        let ids = Set(apps.compactMap(\.bundleIdentifier))
        let procs = Set(apps.compactMap { $0.executableURL?.lastPathComponent })
        return known.filter { ids.contains($0.bundle) || procs.contains($0.process) }
    }
}

struct Status {
    var running = false
    var accessibility = false
    var keytap = false
}

@MainActor
final class Store: ObservableObject {
    let dir: URL
    private let settingsPath: String
    private let memoryPath: String
    private let statusURL: URL
    /// Contents/Resources của Songngu.app chứa app này (nil khi chạy ngoài bundle).
    let resources: URL?

    @Published var options: [String: Bool] = [:]
    @Published var exclude: [AppEntry] = []
    @Published var tap: [AppEntry] = []
    @Published var tapAuto: Set<String> = []
    @Published var builtinTap: [AppEntry] = []
    @Published var learned: [LearnedWord] = []
    @Published var habits: [Habit] = []
    @Published var status = Status()
    @Published var error: String?
    /// Bộ gõ chạy nền khác đang chạy (chỉ cảnh báo, không tự tắt).
    @Published var conflicts: [Conflict] = []
    private var knownConflicts: [Conflict] = []
    /// Songngu đã có trong danh sách nguồn nhập (System Settings > Keyboard > Text Input).
    @Published var enabled = false
    /// Lần kiểm tra cập nhật gần nhất (update.tsv), và đang kiểm tra/cài.
    @Published var updateInfo = ""
    @Published var checking = false
    /// Trang menu SN yêu cầu mở khi cửa sổ đang chạy sẵn (thông báo phân tán từ bộ gõ).
    @Published var requestedPage: Page?

    private var stamps: [Date?] = [nil, nil, nil]
    private var timer: Timer?
    private var names: [String: AppEntry] = [:]

    init() {
        dir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Songngu")
        settingsPath = dir.appendingPathComponent("settings.tsv").path
        memoryPath = dir.appendingPathComponent("memory.tsv").path
        statusURL = dir.appendingPathComponent("status.tsv")
        var url = Bundle.main.bundleURL
        while url.path != "/" && url.lastPathComponent != "Songngu.app" { url.deleteLastPathComponent() }
        resources = url.lastPathComponent == "Songngu.app" ? url.appendingPathComponent("Contents/Resources") : nil
        knownConflicts = Conflict.load(resources)
        reload(force: true)
        // bộ gõ hoặc menu SN có thể sửa file trong lúc cửa sổ đang mở
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.reload(force: false) }
        }
        DistributedNotificationCenter.default().addObserver(forName: Store.openPage, object: nil, queue: .main) { [weak self] n in
            guard let page = (n.object as? String).flatMap(Page.init(rawValue:)) else { return }
            Task { @MainActor in
                self?.requestedPage = page
                NSApp.activate(ignoringOtherApps: true)
            }
        }
    }

    static let openPage = Notification.Name("com.songngu.settings.openPage")

    private static func mtime(_ path: String) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
    }

    private static func take(_ p: UnsafeMutablePointer<CChar>?) -> String {
        guard let p else { return "" }
        defer { sn_str_free(p) }
        return String(cString: p)
    }

    private func list(_ name: String) -> [String] {
        Store.take(sn_cfg_list(settingsPath, name)).split(separator: "\n").map(String.init)
    }

    private func entry(_ bundle: String) -> AppEntry {
        if let e = names[bundle] { return e }
        let e = AppEntry.of(bundle)
        names[bundle] = e
        return e
    }

    func reload(force: Bool) {
        let now = [Store.mtime(settingsPath), Store.mtime(memoryPath), Store.mtime(statusURL.path)]
        let status = readStatus()
        let running = Conflict.running(knownConflicts)
        if running.map(\.bundle) != conflicts.map(\.bundle) { conflicts = running }
        let on = Store.inputSourceEnabled()
        if on != enabled { enabled = on }
        if !force && now == stamps {
            // pid trong status.tsv có thể vừa tắt mà file không đổi
            if status.running != self.status.running { self.status = status }
            return
        }
        if force || now[0] != stamps[0] {
            options = Dictionary(uniqueKeysWithValues: Options.all.map { ($0.key, sn_cfg_option(settingsPath, $0.key)) })
            exclude = list("exclude").map(entry)
            tap = list("tap").map(entry)
            tapAuto = Set(list("tap_auto"))
            builtinTap = list("builtin_tap").map(entry)
        }
        if force || now[1] != stamps[1] {
            learned = Store.take(sn_mem_learned(memoryPath)).split(separator: "\n").compactMap { line in
                let f = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
                guard f.count == 5 else { return nil }
                var detail = "chọn \(f[2]) · bỏ \(f[3])"
                if !f[4].isEmpty { detail += " · sau: \(f[4])" }
                return LearnedWord(key: f[0], word: f[1], detail: detail)
            }
            habits = Store.take(sn_mem_habits(memoryPath)).split(separator: "\n").compactMap { line in
                let f = line.split(separator: "\t").map(String.init)
                guard f.count == 3, let t = Double(f[1]), let l = Double(f[2]) else { return nil }
                return Habit(name: f[0], total: t, late: l)
            }
        }
        self.status = status
        stamps = now
        updateInfo = Updater.lastCheck().map { last in
            let when = last.date.formatted(date: .abbreviated, time: .shortened)
            return "Kiểm tra lúc \(when): \(last.result)"
        } ?? "Chưa kiểm tra lần nào"
    }

    /// Các bước cài đặt mà macOS bắt người dùng tự làm đã xong (trang Bắt đầu).
    var setupDone: Bool { enabled && status.running && status.accessibility && conflicts.isEmpty }

    private static func inputSourceEnabled() -> Bool {
        let filter = [kTISPropertyBundleID as String: "com.songngu.inputmethod.Songngu"] as CFDictionary
        guard let list = TISCreateInputSourceList(filter, false)?.takeRetainedValue() as? [TISInputSource] else { return false }
        return !list.isEmpty
    }

    private func readStatus() -> Status {
        let text = (try? String(contentsOf: statusURL, encoding: .utf8)) ?? ""
        var fields: [String: String] = [:]
        for line in text.split(separator: "\n") {
            let kv = line.split(separator: "\t", maxSplits: 1).map(String.init)
            if kv.count == 2 { fields[kv[0]] = kv[1] }
        }
        guard let pid = fields["pid"].flatMap({ pid_t($0) }), kill(pid, 0) == 0 || errno == EPERM else { return Status() }
        return Status(running: true, accessibility: fields["accessibility"] == "1", keytap: fields["keytap"] == "1")
    }

    // ---- sửa ----

    private func done(_ ok: Bool, _ what: String) {
        error = ok ? nil : "Không ghi được \(what)."
        reload(force: true)
    }

    func toggle(_ key: String) {
        done(sn_cfg_toggle_option(settingsPath, key), "settings.tsv")
        // báo khi đã tự cập nhật xong: xin quyền thông báo lúc người dùng bật
        if key == "auto_update" && options[key] == true {
            UNUserNotificationCenter.current().requestAuthorization(options: [.alert]) { _, _ in }
        }
    }

    /// Kiểm tra và cài ngay nếu có bản mới; cài xong thì mở lại cửa sổ từ bản mới.
    func checkUpdate() {
        checking = true
        Task {
            let result = await Updater.checkAndInstall()
            checking = false
            reload(force: true)
            guard result.hasPrefix("Đã cập nhật") else { return }
            let cfg = NSWorkspace.OpenConfiguration()
            cfg.createsNewApplicationInstance = true
            NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: cfg) { _, _ in
                DispatchQueue.main.async { NSApp.terminate(nil) }
            }
        }
    }

    func remove(_ list: String, _ bundle: String) {
        done(sn_cfg_set(settingsPath, list, bundle, false), "settings.tsv")
    }

    /// Chọn một app trong /Applications rồi thêm vào `list` ("exclude" hoặc "tap").
    func pickApp(into list: String) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.application]
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        panel.prompt = "Thêm"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        guard let id = Bundle(url: url)?.bundleIdentifier else {
            error = "Không đọc được bundle id của \(url.lastPathComponent)."
            return
        }
        done(sn_cfg_set(settingsPath, list, id, true), "settings.tsv")
    }

    func forget(_ key: String) {
        done(sn_mem_forget(memoryPath, key), "memory.tsv")
    }

    func forgetAll() {
        done(sn_mem_forget_all(memoryPath), "memory.tsv")
    }

    // ---- thông tin ----

    var version: String {
        let v = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        guard let c = Bundle.main.object(forInfoDictionaryKey: "SongnguCommit") as? String, !c.isEmpty else { return v }
        return "\(v) (\(c))"
    }

    var model: String {
        guard let r = resources,
              let size = (try? FileManager.default.attributesOfItem(atPath: r.appendingPathComponent("model.bin").path))?[.size] as? Int64
        else { return "Không có: bộ gõ chạy không ngữ cảnh" }
        let tag = (try? String(contentsOf: r.appendingPathComponent("model-version.txt"), encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? "?"
        return "\(tag) · \(ByteCountFormatter.string(fromByteCount: size, countStyle: .file))"
    }

    var notice: URL? {
        resources.map { $0.appendingPathComponent("NOTICE") }.flatMap { FileManager.default.fileExists(atPath: $0.path) ? $0 : nil }
    }

    func openAccessibility() {
        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
    }

    func openTextInput() {
        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension")!)
    }

    /// Hiện Songngu.app trong Finder để kéo vào danh sách Trợ năng khi nó chưa có ở đó.
    func revealApp() {
        guard let app = resources?.deletingLastPathComponent().deletingLastPathComponent() else { return }
        NSWorkspace.shared.activateFileViewerSelecting([app])
    }

    func openLoginItems() {
        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.LoginItems-Settings.extension")!)
    }
}

/// Tuỳ chọn gõ: tên trong settings.tsv, tiêu đề, giải thích.
struct Options: Identifiable {
    let key: String
    let title: String
    let desc: String
    var id: String { key }

    static let style = [
        Options(key: "classic_tone", title: "Đặt dấu kiểu cũ", desc: "hòa, thủy thay vì hoà, thuỷ"),
        Options(key: "vni", title: "Gõ VNI", desc: "Phím số làm dấu: a1 = á, o7 = ơ, d9 = đ"),
        Options(key: "brackets", title: "[ ] gõ ơ ư", desc: "[ = ơ, ] = ư"),
    ]
    static let flip = Options(key: "esc_flip", title: "Lật bằng Esc thay cho Backspace",
                              desc: "Backspace chỉ còn là xoá; Esc lật từ đang gõ và từ vừa gõ ở app thường")
    static let manualTap = Options(key: "manual_tap", title: "Chỉ chặn phím ở app trong danh sách",
                                   desc: "Không tự chặn phím ở app không đưa phím qua bộ gõ (terminal kiểu gpui, vài app Flutter)")
    static let log = Options(key: "log", title: "Ghi nhật ký gõ",
                             desc: "Ghi phím và chữ ra typing-log.tsv trên máy này để báo lỗi; không ghi trong ô mật khẩu")
    static let autoUpdate = Options(key: "auto_update", title: "Tự động cập nhật",
                                    desc: "Mỗi ngày kiểm tra một lần; có bản mới thì tải về, kiểm chữ ký rồi cài.")
    static let all = style + [flip, manualTap, log, autoUpdate]
}
