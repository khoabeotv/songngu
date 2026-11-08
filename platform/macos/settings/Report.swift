// Trang Báo lỗi: gom mô tả, vài dòng nhật ký gõ cuối và thông tin máy thành một issue GitHub cho người dùng xem và sửa
// trước khi gửi. Chỉ mở trình duyệt khi người dùng bấm; bộ gõ không gửi gì ra mạng.
import AppKit
import SwiftUI

enum Report {
    static let repo = "https://github.com/khoabeotv/songngu"
    /// GitHub cắt URL quá dài; dài hơn thì chép nội dung vào clipboard và mở trang issue trống.
    static let maxURL = 7000

    /// Một dòng typing-log.tsv dễ đọc: "tieengs → tiếng", từ tiếng Anh thêm "[En]", lần lật "↺ lật: a → b",
    /// đổi app "— Tên app —". nil với dòng không hiểu.
    static func pretty(_ line: Substring) -> String? {
        let f = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
        switch (f.first, f.count) {
        case ("app", 2): return "— \(AppEntry.of(f[1]).name) (\(f[1])) —"
        case ("flip", 3): return "   ↺ lật: \(f[1]) → \(f[2])"
        case (_, 3): return "\(f[0]) → \(f[1])" + (f[2] == "En" ? "  [En]" : "")
        default: return nil
        }
    }

    /// Dòng đổi app: 2 cột `app<TAB>bundle id` (dòng gõ luôn 3 cột, kể cả khi gõ chữ "app").
    static func appLine(_ line: Substring) -> String? {
        let f = line.split(separator: "\t", omittingEmptySubsequences: false)
        return f.count == 2 && f[0] == "app" ? String(f[1]) : nil
    }

    /// `words` dòng gõ cuối, kèm dòng đổi app gần nhất trước đó để biết lỗi ở app nào. Trả cả bundle id app cuối.
    static func tail(_ url: URL, words: Int) -> (text: String, app: String?) {
        guard let all = try? String(contentsOf: url, encoding: .utf8) else { return ("", nil) }
        let lines = all.split(separator: "\n")
        var start = lines.endIndex
        var n = 0
        while start > lines.startIndex && n < words {
            start -= 1
            if appLine(lines[start]) == nil { n += 1 }
        }
        let lastApp = lines.lazy.reversed().compactMap(appLine).first
        var picked = Array(lines[start...])
        if picked.first.flatMap(appLine) == nil,
           let app = lines[..<start].last(where: { appLine($0) != nil }) {
            picked.insert(app, at: 0)
        }
        return (picked.compactMap(pretty).joined(separator: "\n"), lastApp)
    }

    /// Mã hoá chặt (cả "+", "&"): GitHub đọc "+" trong query là dấu cách.
    static func newIssue(title: String, body: String) -> URL? {
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
        func enc(_ s: String) -> String { s.addingPercentEncoding(withAllowedCharacters: allowed) ?? "" }
        return URL(string: "\(repo)/issues/new?title=\(enc(title))&body=\(enc(body))")
    }

    static var machine: String {
        var info = utsname()
        uname(&info)
        let arch = withUnsafeBytes(of: &info.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "macOS \(v.majorVersion).\(v.minorVersion).\(v.patchVersion), \(arch)"
    }
}

struct ReportPage: View {
    @EnvironmentObject var store: Store
    @State private var what = ""
    @State private var app = ""
    @State private var log = ""
    @State private var words = 30
    @State private var sent: String?

    private var logging: Bool { store.options["log"] ?? false }

    private var info: String {
        let on = Options.all.filter { store.options[$0.key] == true }.map(\.key)
        let s = store.status
        return """
        - Songngu: \(store.version)
        - Máy: \(Report.machine)
        - Mô hình: \(store.model)
        - Tuỳ chọn đang bật: \(on.isEmpty ? "không" : on.joined(separator: ", "))
        - Trợ năng: \(s.running ? (s.accessibility ? "có" : "chưa") : "không rõ (bộ gõ chưa chạy)"), chặn phím: \(s.keytap ? "đang chạy" : "tắt")
        """
    }

    private var issueBody: String {
        var b = "**Mô tả**\n\n\(what.isEmpty ? "_(chưa mô tả)_" : what)\n\n"
        if !app.isEmpty { b += "**App**: \(app)\n\n" }
        if !log.isEmpty { b += "**Nhật ký gõ** (phím → chữ)\n\n```\n\(log)\n```\n\n" }
        return b + "**Máy**\n\n\(info)\n"
    }

    private var title: String {
        let first = what.split(separator: "\n").first.map(String.init) ?? ""
        return "Lỗi gõ: " + (first.isEmpty ? "(chưa đặt tên)" : String(first.prefix(70)))
    }

    var body: some View {
        Form {
            Section {
                TextEditor(text: $what)
                    .font(.body)
                    .frame(minHeight: 70)
                    .overlay(alignment: .topLeading) {
                        if what.isEmpty {
                            Text("Đã gõ gì, mong ra chữ gì, thực tế ra chữ gì. Ví dụ: gõ \"truowfng\" ra \"truoư\" thay vì \"trường\".")
                                .foregroundStyle(.tertiary)
                                .padding(.leading, 5)
                                .allowsHitTesting(false)
                        }
                    }
                TextField("App đang gõ", text: $app, prompt: Text("Ví dụ: Zalo, Ghostty, Chrome"))
            } header: {
                Text("Lỗi gặp phải")
            }
            Section {
                if logging {
                    Stepper("Số dòng gõ gần nhất: \(words)", value: $words, in: 10...100, step: 10)
                        .onChange(of: words) { _ in loadLog() }
                    TextEditor(text: $log)
                        .font(.system(.callout, design: .monospaced))
                        .frame(minHeight: 160)
                    HStack {
                        Button("Lấy lại nhật ký") { loadLog() }
                        Button("Bỏ nhật ký") { log = "" }
                    }
                } else {
                    OptionToggle(option: Options.log)
                    Text("Bật rồi gõ lại đúng chỗ bị sai, sau đó quay lại trang này: nhật ký giúp tìm ra lỗi nhanh hơn nhiều.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            } header: {
                Text("Nhật ký gõ")
            } footer: {
                Text("Issue trên GitHub là công khai. Xoá mọi dòng có nội dung riêng tư (tin nhắn, tên, số tài khoản) trước khi gửi; chỉ cần giữ vài dòng quanh chữ bị sai.")
            }
            Section("Thông tin máy (gửi kèm)") {
                Text(info)
                    .font(.system(.callout, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Section {
                HStack {
                    Button("Mở issue trên GitHub") { open() }
                        .buttonStyle(.borderedProminent)
                        .disabled(what.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Button("Chép nội dung") { copy() }
                    Spacer()
                    if let sent { Text(sent).font(.subheadline).foregroundStyle(.secondary) }
                }
            } footer: {
                Text("Cần tài khoản GitHub để gửi. Không có thì bấm \"Chép nội dung\" rồi gửi cho người phát triển qua kênh khác.")
            }
        }
        .formStyle(.grouped)
        .onAppear(perform: loadLog)
    }

    private func loadLog() {
        let t = Report.tail(store.dir.appendingPathComponent("typing-log.tsv"), words: words)
        log = t.text
        if app.isEmpty, let id = t.app { app = "\(AppEntry.of(id).name) (\(id))" }
    }

    private func copy() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString("# \(title)\n\n\(issueBody)", forType: .string)
        sent = "Đã chép vào clipboard"
    }

    private func open() {
        if let url = Report.newIssue(title: title, body: issueBody), url.absoluteString.count <= Report.maxURL {
            NSWorkspace.shared.open(url)
            sent = "Đã mở trình duyệt"
            return
        }
        copy()
        if let url = Report.newIssue(title: title, body: "(Nội dung dài, đã chép vào clipboard: bấm Cmd+V để dán vào đây.)") {
            NSWorkspace.shared.open(url)
        }
        sent = "Nội dung dài: đã chép vào clipboard, dán vào trang vừa mở"
    }
}
