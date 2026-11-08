// Bốn trang của cửa sổ cài đặt, theo kiểu System Settings: Form nhóm, control gốc, mô tả nhỏ màu phụ.
import AppKit
import SwiftUI

/// Tiêu đề + mô tả nhỏ, như nhãn trong System Settings.
struct Titled: View {
    let title: String
    let desc: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if let desc {
                Text(desc).font(.subheadline).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct OptionToggle: View {
    @EnvironmentObject var store: Store
    let option: Options

    var body: some View {
        Toggle(isOn: Binding(get: { store.options[option.key] ?? false }, set: { _ in store.toggle(option.key) })) {
            Titled(title: option.title, desc: option.desc)
        }
    }
}

struct TypingPage: View {
    @EnvironmentObject var store: Store

    var body: some View {
        Form {
            Section("Kiểu gõ") {
                ForEach(Options.style) { OptionToggle(option: $0) }
            }
            Section {
                OptionToggle(option: Options.flip)
            } header: {
                Text("Lật Anh/Việt")
            } footer: {
                Text("Mặc định: bấm Backspace ngay sau từ để lật nó. Ctrl+Esc hoặc Shift+Esc lật được ở mọi app.")
            }
            Section("Chặn phím") {
                OptionToggle(option: Options.manualTap)
            }
            Section("Gỡ lỗi") {
                OptionToggle(option: Options.log)
                LabeledContent("Thư mục nhật ký") {
                    Button("Mở trong Finder") { NSWorkspace.shared.open(store.dir) }
                }
            }
        }
        .formStyle(.grouped)
    }
}

struct AppRow: View {
    let app: AppEntry
    var auto = false
    var remove: (() -> Void)?

    var body: some View {
        HStack(spacing: 10) {
            Image(nsImage: app.icon).resizable().frame(width: 24, height: 24)
            Titled(title: app.name, desc: app.name == app.bundle ? nil : app.bundle)
            Spacer()
            if auto {
                Text("Tự phát hiện")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(.quaternary, in: Capsule())
            }
            if let remove {
                Button("Gỡ", action: remove)
            }
        }
    }
}

struct AppsPage: View {
    @EnvironmentObject var store: Store

    var body: some View {
        Form {
            Section {
                if store.exclude.isEmpty {
                    Text("Chưa có app nào").foregroundStyle(.secondary)
                }
                ForEach(store.exclude) { app in
                    AppRow(app: app) { store.remove("exclude", app.bundle) }
                }
            } header: {
                header("Bỏ qua") { store.pickApp(into: "exclude") }
            } footer: {
                Text("Songngu đứng ngoài các app này, phím đi thẳng vào app.")
            }
            Section {
                if store.tap.isEmpty {
                    Text("Chưa có app nào").foregroundStyle(.secondary)
                }
                ForEach(store.tap) { app in
                    AppRow(app: app, auto: store.tapAuto.contains(app.bundle)) { store.remove("tap", app.bundle) }
                }
            } header: {
                header("Chặn phím") { store.pickApp(into: "tap") }
            } footer: {
                Text("Sửa chữ bằng Backspace giả thay vì nhờ app thay chữ, cho app gõ ra chữ lặp hoặc sai dấu. App \"Tự phát hiện\" do bộ gõ tự thêm khi thấy app không thay chữ được: gỡ nếu app vẫn gõ đúng.")
            }
            Section("Luôn chặn phím (có sẵn)") {
                ForEach(store.builtinTap) { AppRow(app: $0) }
            }
        }
        .formStyle(.grouped)
    }

    private func header(_ title: String, add: @escaping () -> Void) -> some View {
        HStack {
            Text(title)
            Spacer()
            Button("Thêm app…", action: add).controlSize(.small)
        }
    }
}

struct LearnedPage: View {
    @EnvironmentObject var store: Store
    @State private var confirm = false

    var body: some View {
        Form {
            Section {
                if store.learned.isEmpty {
                    Text("Chưa học từ nào.").foregroundStyle(.secondary)
                }
                ForEach(store.learned) { w in
                    HStack {
                        Titled(title: w.word, desc: w.detail)
                        Spacer()
                        Button("Quên") { store.forget(w.key) }
                    }
                }
            } header: {
                HStack {
                    Text("\(store.learned.count) từ")
                    Spacer()
                    Button("Quên tất cả…", role: .destructive) { confirm = true }
                        .controlSize(.small)
                        .disabled(store.learned.isEmpty && store.habits.isEmpty)
                }
            } footer: {
                Text("Mỗi lần lật, Songngu nhớ từ được chọn và từ đứng trước nó. Quên một từ nếu bộ gõ học nhầm.")
            }
            Section {
                if store.habits.isEmpty {
                    Text("Chưa gõ đủ để biết.").foregroundStyle(.secondary)
                }
                ForEach(store.habits) { h in
                    LabeledContent {
                        Text(h.total < 20 ? "Chưa đủ dữ liệu" : "\(Int((100 * h.late / h.total).rounded()))%")
                            .foregroundStyle(.secondary)
                    } label: {
                        Titled(title: h.title, desc: "\(Int(h.late)) trong \(Int(h.total)) từ \(h.name == "late" ? "có â, ê, ô" : "có đ")")
                    }
                }
            } header: {
                Text("Thói quen gõ")
            } footer: {
                Text("Songngu đếm bạn hay gõ dấu muộn đến đâu để bớt đoán nhầm: quen gõ \"cana\" cho \"cân\" thì bộ gõ dễ ra chữ Việt hơn. Chỉ lưu các con số này, không lưu chữ.")
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("Quên hết những gì Songngu đã học?", isPresented: $confirm) {
            Button("Quên hết", role: .destructive) { store.forgetAll() }
            Button("Huỷ", role: .cancel) {}
        } message: {
            Text("File cũ được giữ ở memory-forgotten.tsv (thay bản giữ lần trước).")
        }
    }
}

/// Trạng thái có/không; bộ gõ chưa chạy thì chưa biết (trạng thái do chính bộ gõ ghi).
struct Badge: View {
    let known: Bool
    let ok: Bool
    let yes: String
    let no: String

    var body: some View {
        if !known {
            Label("Chưa rõ", systemImage: "questionmark.circle").foregroundStyle(.secondary)
        } else if ok {
            Label(yes, systemImage: "checkmark.circle.fill").foregroundStyle(.green)
        } else {
            Label(no, systemImage: "xmark.circle.fill").foregroundStyle(.red)
        }
    }
}

struct StatusPage: View {
    @EnvironmentObject var store: Store

    var body: some View {
        let s = store.status
        Form {
            Section {
                LabeledContent {
                    Badge(known: true, ok: s.running, yes: "Đang chạy", no: "Chưa chạy")
                } label: {
                    Titled(title: "Bộ gõ", desc: "Chọn Songngu trong Input Sources (Ctrl+Space) để bộ gõ chạy.")
                }
                LabeledContent {
                    HStack {
                        Badge(known: s.running, ok: s.accessibility, yes: "Đã cấp", no: "Chưa cấp")
                        if s.running && !s.accessibility {
                            Button("Mở cài đặt Trợ năng") { store.openAccessibility() }
                        }
                    }
                } label: {
                    Titled(title: "Quyền Trợ năng", desc: "Cần để gõ trong terminal và app không nhận bộ gõ.")
                }
                LabeledContent {
                    Badge(known: s.running, ok: s.keytap, yes: "Đang chạy", no: "Tắt")
                } label: {
                    Titled(title: "Chặn phím", desc: "Sửa chữ bằng Backspace giả, chạy khi có quyền Trợ năng.")
                }
            }
            Section {
                if store.conflicts.isEmpty {
                    Label("Không thấy bộ gõ chạy nền nào khác", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                }
                ForEach(store.conflicts) { c in
                    VStack(alignment: .leading, spacing: 6) {
                        Label("\(c.name) đang chạy", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                            .font(.body.weight(.semibold))
                        Text("\(c.name) bắt phím toàn hệ thống nên chạy cùng Songngu sẽ ra dấu đôi, mất hoặc nhảy chữ. Thoát \(c.name) (biểu tượng của nó trên thanh menu > Thoát), bỏ nó khỏi danh sách mở cùng máy, và tắt nó trong quyền Trợ năng.")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                        HStack {
                            Button("Mở Login Items") { store.openLoginItems() }
                            Button("Mở Trợ năng") { store.openAccessibility() }
                        }
                    }
                    .padding(.vertical, 2)
                }
            } header: {
                Text("Bộ gõ khác")
            } footer: {
                Text("Songngu chỉ cảnh báo, không tự tắt hay gỡ app nào. Bộ gõ nằm trong Input Sources (Telex có sẵn của macOS, XKeyIM...) không xung đột vì mỗi lúc chỉ một nguồn nhập được chọn.")
            }
            Section {
                OptionToggle(option: Options.autoUpdate)
                LabeledContent {
                    Button(store.checking ? "Đang kiểm tra…" : "Kiểm tra ngay") { store.checkUpdate() }
                        .disabled(store.checking)
                } label: {
                    Titled(title: "Phiên bản \(store.version)", desc: store.updateInfo)
                }
            } header: {
                Text("Cập nhật")
            } footer: {
                Text("Chỉ cửa sổ cài đặt kết nối GitHub để kiểm tra và tải bản mới; bộ gõ không truy cập mạng. Bản mới phải ký cùng chứng chỉ với bản đang cài.")
            }
            Section("Thông tin") {
                LabeledContent("Mô hình ngữ cảnh", value: store.model)
                LabeledContent {
                    Button("Mở trong Finder") { NSWorkspace.shared.open(store.dir) }
                } label: {
                    Titled(title: "Dữ liệu người dùng", desc: store.dir.path)
                }
                LabeledContent {
                    if let notice = store.notice {
                        Button("Xem NOTICE") { NSWorkspace.shared.open(notice) }
                    }
                } label: {
                    Titled(title: "Giấy phép",
                           desc: "Mã nguồn MIT. Từ điển và mô hình CC BY-SA 4.0 (FrequencyWords, FineWeb-2, tài liệu React/Vue/Kubernetes).")
                }
            }
        }
        .formStyle(.grouped)
    }
}

/// Một bước của trang Bắt đầu: số thứ tự (xong thì dấu tick), mô tả, nút làm bước đó khi chưa xong.
struct Step<Actions: View>: View {
    let number: Int
    let done: Bool
    /// Chưa biết được (cần bước trước xong).
    var unknown = false
    let title: String
    let desc: String
    @ViewBuilder let actions: () -> Actions

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: done ? "checkmark.circle.fill" : unknown ? "circle.dashed" : "\(number).circle")
                .font(.title2)
                .foregroundStyle(done ? AnyShapeStyle(.green) : AnyShapeStyle(.secondary))
                .frame(width: 26)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.body.weight(.medium))
                Text(desc)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if !done {
                    HStack { actions() }.padding(.top, 2)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
    }
}

/// Các bước macOS bắt người dùng tự làm sau khi cài; tự đánh dấu khi xong (kiểm mỗi giây).
struct StartPage: View {
    @EnvironmentObject var store: Store

    var body: some View {
        let s = store.status
        let others = store.conflicts.map(\.name).joined(separator: ", ")
        Form {
            Section {
                Step(number: 1, done: store.enabled, title: "Thêm Songngu vào nguồn nhập",
                     desc: "Keyboard > Text Input > Edit… > + > Vietnamese > Songngu > Add.") {
                    Button("Mở cài đặt Bàn phím") { store.openTextInput() }
                }
                Step(number: 2, done: s.running, title: "Chọn SN",
                     desc: "Bấm Ctrl+Space hoặc chọn SN ở biểu tượng nguồn nhập trên thanh menu, rồi gõ thử ở bất kỳ đâu.") {
                    EmptyView()
                }
                Step(number: 3, done: s.accessibility, unknown: !s.running, title: "Cấp quyền Trợ năng",
                     desc: s.running
                        ? "Cần để gõ trong terminal và app không nhận bộ gõ. Privacy & Security > Accessibility: bật Songngu; chưa có trong danh sách thì kéo Songngu.app vào."
                        : "Làm sau bước 2: lúc đó mới biết bộ gõ đã có quyền chưa.") {
                    if s.running {
                        Button("Mở cài đặt Trợ năng") { store.openAccessibility() }
                        Button("Hiện Songngu.app") { store.revealApp() }
                    }
                }
                Step(number: 4, done: store.conflicts.isEmpty, title: "Tắt bộ gõ chạy nền khác",
                     desc: store.conflicts.isEmpty
                        ? "Không thấy bộ gõ chạy nền nào khác."
                        : "\(others) đang chạy, sẽ ra dấu đôi, mất hoặc nhảy chữ. Thoát nó, bỏ khỏi danh sách mở cùng máy và tắt nó trong quyền Trợ năng.") {
                    Button("Mở Login Items") { store.openLoginItems() }
                    Button("Mở cài đặt Trợ năng") { store.openAccessibility() }
                }
            } header: {
                Text(store.setupDone ? "Đã sẵn sàng" : "Vài bước macOS bắt bạn tự làm")
            } footer: {
                Text("Các bước tự đánh dấu khi xong.")
            }
            if store.setupDone {
                Section("Gõ thử") {
                    LabeledContent("truowfng merge test cos", value: "trường merge test có")
                    LabeledContent("Đoán sai", value: "Backspace ngay sau từ để lật")
                }
            }
        }
        .formStyle(.grouped)
    }
}
