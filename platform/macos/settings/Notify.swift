// Thông báo hệ thống. Bộ gõ là tiến trình nền nên nhờ app này: `SongnguSettings --notify tiêu-đề nội-dung` (KeyTap.m).
import Foundation
import UserNotifications

enum Notify {
    /// `ask`: chưa từng hỏi thì xin quyền (macOS hỏi người dùng một lần); không thì chỉ gửi khi đã được cho phép.
    static func post(title: String, body: String, ask: Bool) async {
        let center = UNUserNotificationCenter.current()
        var status = await center.notificationSettings().authorizationStatus
        if status == .notDetermined && ask {
            _ = try? await center.requestAuthorization(options: [.alert])
            status = await center.notificationSettings().authorizationStatus
        }
        guard status == .authorized else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        try? await center.add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
    }

    /// Chạy ngầm, không mở cửa sổ; chờ tối đa 2 phút (người dùng trả lời hộp xin quyền).
    static func runHeadless(title: String, body: String) -> Int32 {
        var done = false
        Task {
            await post(title: title, body: body, ask: true)
            done = true
        }
        let deadline = Date().addingTimeInterval(120)
        while !done && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.1)) }
        return 0
    }
}
