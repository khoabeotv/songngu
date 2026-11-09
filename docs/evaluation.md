# Đo đạc

## Người gõ giả lập (`src/typing.rs`)

Văn bản đo: mỗi dòng một tin nhắn, đáp án là chính chữ đó (so bỏ qua vị trí dấu thanh). Người gõ giả lập gõ từ tiếng Anh đúng từng chữ, từ tiếng Việt bằng Telex chuẩn (mũ/móc liền sau chữ, dấu thanh cuối từ) và không bao giờ tự chuyển ngôn ngữ.

- `--realistic`: 30% từ có thanh gõ dấu ngay sau nguyên âm (tieesng), 20% từ có ư/ơ/ă gõ w cuối (truongwf), 20% từ có â/ê/ô gõ phím mũ muộn hơn (thayas, canaf), 20% từ có đ gõ d thứ hai muộn hơn (duowdcj), 3% từ gõ nhầm một phím rồi Backspace ngay.
- `--late-hat R`, `--late-w R`, `--late-d-rate R`, `--mid-tone R`: đổi riêng từng tỉ lệ trên (người quen gõ dấu muộn).
- `--swaps R`: tỉ lệ R từ có dấu gõ đảo hai phím liền nhau, vẫn chấm theo chữ đúng.
- `--clicks N` (`eval-session`): cứ N từ giả như click đi rồi quay lại (bộ gõ mất ngữ cảnh, chữ vẫn trong app); `--no-resume` tắt việc đọc lại ngữ cảnh từ chữ trước con trỏ, `--no-adapt` tắt học thói quen gõ.
- `--typos R`: tỉ lệ R từ gõ nhầm không sửa (phím bên cạnh, sót, lặp, đảo phím). Từ đó không chấm vì sai đằng nào người gõ cũng phải sửa; chỉ đo ảnh hưởng lên các từ còn lại.
- `--seed N`: đổi ngẫu nhiên.

Từ sai đo qua lớp bộ gõ (`eval-session`; đường thay chữ và xoá lùi cho cùng số từ sai). Sửa-khi-Space và đổi giữa từ đo ở mức engine (`eval`).

## Kết quả (bộ test)

| Bộ test | Từ | Từ sai | Gõ như người thật | Sửa-khi-Space | Đổi giữa từ |
|---|---|---|---|---|---|
| Kỹ thuật (tài liệu dịch + trang kỹ thuật) | 60k | 0,02% | 0,03% | 0,07% | 0,41% |
| FineWeb (web đời thường) | 73k | 0,06% | 0,06% | 0,11% | 0,44% |
| Chat (Haiku sinh, chủ đề không có trong phần học) | 51k | 0,11% | 0,14% | 0,27% | 2,25% |

- Bỏ chat khỏi phần học: chat test sai 93 từ thay vì 54 (0,18%), kỹ thuật/FineWeb gần như không đổi (12/44 thay vì 11/42).
- Gõ nhầm 5% không sửa: các từ còn lại sai 55/14/40 (chat/kỹ thuật/FineWeb), như khi gõ sạch.
- Gõ dấu muộn đôi khi ra đúng một từ Anh (thêm gõ `theme`, mây gõ `maya`): bộ gõ nghiêng tiếng Anh, Backspace ngay sau từ để lật. Đ gõ muộn (`did` cho "đi") để ngữ cảnh quyết. Phần lớn chênh lệch giữa cột gõ chuẩn và gõ như người thật ở chat là do những từ này.
- Học thói quen gõ: người gõ như thật (20% mũ/đ muộn) sai 342 thay vì 352 từ trên test (dev 334/339, cột gõ chuẩn không đổi); người gõ 60% mũ và đ muộn sai 361 thay vì 478 (dev 370/473). Độ mạnh mức chung 20/50/200 từ cho cùng kết quả ±2; bỏ sàn mức chung được dev 331 nhưng làm người hiếm khi gõ muộn khó gõ muộn hơn, nên giữ sàn.
- Giữ chữ đang hiện khi hai phương án chênh dưới 2 lần lúc chốt: dev 317 -> 307 (Lee -> Lê, õi -> oxi, boots -> bốt; hỏng TIS -> TÍ, Nene -> Nên), test 336 -> 337 (không đổi đáng kể). Ngưỡng 1,5/2/2,5/3/4/10: dev 316/307/307/309/317/328.
- Từ cuối câu (Enter, `?` `!`) so thêm khả năng hết câu: test 346 -> 336, dev 332 -> 317 (as -> á, mã -> max), không từ nào trên dev hỏng thêm. Ngưỡng đổi 0,3-1 cho cùng kết quả, 2/3/10 cho 329/329/332; chọn 1 (chỉ cần phương án kia hợp hơn).
- Sửa gõ đảo hai phím: 5% từ có dấu gõ đảo thì test sai 765 thay vì 3890 từ (dev 789/3659). Gõ thường: test 346 thay vì 342, dev 332 thay vì 334. Phạt 1e-3 sửa thêm ít (dev 733) nhưng đổi cả từ lạ gõ đúng (itm -> tim, ib -> bi); bỏ qua từ có trong danh sách từ Anh của corpus làm mất nửa phần sửa (dev 1251) vì danh sách đó lẫn từ gõ sai.
- Click đi rồi quay lại giữa câu: cứ 8 từ một lần thì test sai 398 từ nếu coi như đầu câu, 362 khi đọc lại ngữ cảnh từ chữ trước con trỏ; cứ 4 từ: 479/403 (dev 380/337, 426/355). Lấy tiếng của 0/3/8/20 từ cuối: dev 357/356/355/358 (cứ 4 từ), chọn 8.
- Ngữ cảnh học từ cùng nguồn với hai bộ test đầu (khác tài liệu); chat test cùng model sinh và prompt với phần học (khác chủ đề). Các số này lạc quan hơn văn bản thật của người dùng.
- Lỗi còn lại chủ yếu là từ có nghĩa ở cả hai thứ tiếng (been/bên, its/ít, car/cả), viết tắt trùng âm tiết Việt (ASO/ÁO, HW/HƯ) và lỗi chính tả có sẵn trong văn bản gốc (đuợc, nguời) mà Telex không gõ ra.
- Người dùng lật ngay từ sai: bộ nhớ riêng giảm số lần phải lật 12% (kỹ thuật dev, 17 -> 15) đến 21% (FineWeb dev, 43 -> 34); dev chat gần như không đổi (21 -> 19, 35 -> 35) (`eval --fix` so với `--no-learn`).
- Từ còn phân vân (Esc lật được ở từ trước con trỏ): 0,6-1,4% số từ sau khi gõ xong, 1,3-3,3% số phím lúc đang gõ dở (`eval-session`: unsure words, unsure keys).
- Tốc độ engine ~2 µs/phím, p99 ~5 µs; gõ đều 10 phím/giây ~100 µs/phím (bộ nhớ đệm nguội giữa hai phím; `target/bench` qua C ABI). Mô hình 22 MB, nạp ~1 ms; tiến trình bộ gõ ~9 MB footprint (gồm AppKit).

## Đã thử, không dùng

- Bộ xếp hạng lại bằng mạng nhỏ (`tools/eval/rerank_train.py`: embedding băm của 4 từ trước + 3 từ sau, học trên ví dụ `ambig` chấm chéo hai nửa corpus) cho từ phân vân: không hơn bước nhìn từ sau của bộ gõ trên dev. Lỗi còn lại cần hiểu nghĩa câu, tức một mô hình ngôn ngữ cỡ vài chục MB.
- Trigram: xem `architecture.md`.
- Tự củng cố từ phân vân người dùng giữ nguyên (không lật), trọng số 0,1/0,3/1 lần lật: dev 335/339/332 so với 334, không ổn định. Còn củng cố cả từ sai người dùng không buồn sửa và làm `memory.tsv` chứa chữ đã gõ, nên không dùng.
