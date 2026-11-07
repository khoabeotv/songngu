// Tự cập nhật bản build sẵn (cài bằng install.sh). Chỉ app cài đặt này truy cập mạng; bộ gõ thì không, nó chỉ gọi
// `SongnguSettings --update` mỗi giờ khi bật tuỳ chọn auto_update (ở đây tự giới hạn mỗi ngày một lần).
// Bản mới phải ký cùng chứng chỉ với bản đang cài (designated requirement của Songngu.app hiện tại): ai chiếm được
// GitHub cũng không đẩy được bản giả, vì khoá ký nằm trên máy phát hành. Bản đang cài ký ad-hoc thì không tự cập nhật.
import AppKit
import CryptoKit
import Security
import UserNotifications

struct UpdateError: Error, CustomStringConvertible {
    let description: String
    init(_ d: String) { description = d }
}

enum Updater {
    /// SONGNGU_UPDATE_BASE thay địa chỉ GitHub để thử (file:///… có cùng cấu trúc releases/…).
    static var base: String {
        ProcessInfo.processInfo.environment["SONGNGU_UPDATE_BASE"] ?? "https://github.com/khoabeotv/songngu"
    }

    static let imeBundleID = "com.songngu.inputmethod.Songngu"
    static let minInterval: TimeInterval = 20 * 3600

    struct Release {
        let version: String
        let sha256: String
    }

    /// Songngu.app chứa app này (nil khi chạy ngoài bundle).
    static var installedApp: URL? {
        var url = Bundle.main.bundleURL
        while url.path != "/" && url.lastPathComponent != "Songngu.app" { url.deleteLastPathComponent() }
        return url.lastPathComponent == "Songngu.app" ? url : nil
    }

    static var currentVersion: String {
        installedApp.flatMap { Bundle(url: $0)?.infoDictionary?["CFBundleShortVersionString"] as? String } ?? "0"
    }

    /// "0.1.10" > "0.1.9"
    static func isNewer(_ a: String, than b: String) -> Bool {
        let pa = a.split(separator: ".").map { Int($0) ?? 0 }, pb = b.split(separator: ".").map { Int($0) ?? 0 }
        for i in 0..<max(pa.count, pb.count) {
            let (x, y) = (i < pa.count ? pa[i] : 0, i < pb.count ? pb[i] : 0)
            if x != y { return x > y }
        }
        return false
    }

    private static func fetch(_ url: URL) async throws -> Data {
        let (data, resp) = try await URLSession.shared.data(from: url)
        if let http = resp as? HTTPURLResponse, http.statusCode != 200 {
            throw UpdateError("máy chủ trả \(http.statusCode)")
        }
        return data
    }

    /// Bản mới nhất: đọc phiên bản và checksum mà release.sh điền vào install.sh của release Latest.
    static func latest() async throws -> Release {
        let text = String(decoding: try await fetch(URL(string: base + "/releases/latest/download/install.sh")!), as: UTF8.self)
        func field(_ name: String) -> String? {
            text.split(separator: "\n").lazy.compactMap { line -> String? in
                guard line.hasPrefix(name + "=\"") else { return nil }
                return String(line.dropFirst(name.count + 2).dropLast())
            }.first
        }
        guard let v = field("VERSION"), let s = field("SHA256"), s.count == 64 else { throw UpdateError("không đọc được bản mới nhất") }
        return Release(version: v, sha256: s)
    }

    private static func run(_ tool: String, _ args: [String]) throws {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: tool)
        p.arguments = args
        try p.run()
        p.waitUntilExit()
        guard p.terminationStatus == 0 else { throw UpdateError("\(tool) lỗi \(p.terminationStatus)") }
    }

    /// `new` thoả designated requirement của `current`: cùng bundle id, cùng chứng chỉ ký.
    static func verifySameSigner(_ new: URL, as current: URL) throws {
        var cur: SecStaticCode?
        var req: SecRequirement?
        var nw: SecStaticCode?
        guard SecStaticCodeCreateWithPath(current as CFURL, [], &cur) == errSecSuccess, let cur,
              SecCodeCopyDesignatedRequirement(cur, [], &req) == errSecSuccess, let req,
              SecStaticCodeCreateWithPath(new as CFURL, [], &nw) == errSecSuccess, let nw
        else { throw UpdateError("không đọc được chữ ký") }
        var text: CFString?
        SecRequirementCopyString(req, [], &text)
        guard (text as String?)?.contains("certificate") == true else {
            throw UpdateError("bản đang cài không ký bằng chứng chỉ (build từ mã nguồn?), không tự cập nhật")
        }
        let flags = SecCSFlags(rawValue: kSecCSCheckAllArchitectures | kSecCSCheckNestedCode | kSecCSStrictValidate)
        guard SecStaticCodeCheckValidity(nw, flags, req) == errSecSuccess else {
            throw UpdateError("bản tải về không ký cùng chứng chỉ, bỏ qua")
        }
    }

    /// Tải, kiểm checksum và chữ ký, thay Songngu.app, tắt bộ gõ cũ (hệ thống mở bản mới ở phím gõ tiếp theo).
    static func install(_ r: Release) async throws {
        guard let app = installedApp else { throw UpdateError("không chạy từ Songngu.app") }
        let zipURL = URL(string: base + "/releases/download/v\(r.version)/Songngu-\(r.version).zip")!
        let data = try await fetch(zipURL)
        let sum = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard sum == r.sha256 else { throw UpdateError("file tải về sai checksum") }
        let fm = FileManager.default
        let work = fm.temporaryDirectory.appendingPathComponent("songngu-update-\(UUID().uuidString)")
        defer { try? fm.removeItem(at: work) }
        try fm.createDirectory(at: work, withIntermediateDirectories: true)
        let zip = work.appendingPathComponent("Songngu.zip")
        try data.write(to: zip)
        try run("/usr/bin/ditto", ["-x", "-k", zip.path, work.path])
        let new = work.appendingPathComponent("Songngu.app")
        try verifySameSigner(new, as: app)
        try? run("/usr/bin/xattr", ["-dr", "com.apple.quarantine", new.path])
        // đổi tên rồi mới xoá: lỗi giữa chừng thì trả bản cũ về chỗ cũ
        let old = app.deletingLastPathComponent().appendingPathComponent(".Songngu.app.old")
        try? fm.removeItem(at: old)
        try fm.moveItem(at: app, to: old)
        do {
            try fm.moveItem(at: new, to: app)
        } catch {
            try? fm.moveItem(at: old, to: app)
            throw error
        }
        try? fm.removeItem(at: old)
        for ime in NSRunningApplication.runningApplications(withBundleIdentifier: imeBundleID) { ime.forceTerminate() }
    }

    // ---- nhật ký kiểm tra (update.tsv, cửa sổ cài đặt hiện ở trang Trạng thái) ----

    static var logURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Songngu/update.tsv")
    }

    static func lastCheck() -> (date: Date, result: String)? {
        guard let text = try? String(contentsOf: logURL, encoding: .utf8) else { return nil }
        var f: [String: String] = [:]
        for line in text.split(separator: "\n") {
            let kv = line.split(separator: "\t", maxSplits: 1).map(String.init)
            if kv.count == 2 { f[kv[0]] = kv[1] }
        }
        guard let t = f["checked"].flatMap(TimeInterval.init) else { return nil }
        return (Date(timeIntervalSince1970: t), f["result"] ?? "")
    }

    static func record(_ result: String) {
        let text = "checked\t\(Int(Date().timeIntervalSince1970))\nresult\t\(result)\n"
        try? FileManager.default.createDirectory(at: logURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? text.write(to: logURL, atomically: true, encoding: .utf8)
    }

    /// Kiểm tra và cài nếu có bản mới. Trả câu báo kết quả (đã ghi vào update.tsv).
    @discardableResult
    static func checkAndInstall() async -> String {
        let result: String
        do {
            let r = try await latest()
            if isNewer(r.version, than: currentVersion) {
                try await install(r)
                result = "Đã cập nhật lên \(r.version)"
            } else {
                result = "Đang dùng bản mới nhất (\(currentVersion))"
            }
        } catch {
            result = "Không cập nhật được: \(error)"
        }
        record(result)
        return result
    }

    /// `SongnguSettings --update [--force]`: chạy ngầm, không mở cửa sổ. Không --force thì chỉ chạy khi bật
    /// auto_update và lần kiểm tra trước đã quá `minInterval`.
    static func runHeadless(force: Bool) -> Int32 {
        if !force {
            let settings = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Application Support/Songngu/settings.tsv").path
            guard sn_cfg_option(settings, "auto_update") else { return 0 }
            if let last = lastCheck(), Date().timeIntervalSince(last.date) < minInterval { return 0 }
        }
        var done = false
        Task {
            let result = await checkAndInstall()
            print(result)
            if result.hasPrefix("Đã cập nhật") { await notify(result) }
            done = true
        }
        while !done { RunLoop.main.run(until: Date().addingTimeInterval(0.1)) }
        return 0
    }

    /// Báo cập nhật xong nếu người dùng đã cho phép thông báo (xin quyền lúc bật tự động cập nhật trong cửa sổ).
    static func notify(_ text: String) async {
        let center = UNUserNotificationCenter.current()
        guard await center.notificationSettings().authorizationStatus == .authorized else { return }
        let content = UNMutableNotificationContent()
        content.title = "Songngu"
        content.body = text + "."
        try? await center.add(UNNotificationRequest(identifier: "songngu-update", content: content, trigger: nil))
    }
}
