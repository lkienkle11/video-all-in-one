# Changelog

All notable changes to Video All-in-One are written here, newest first. Every version has an
English part and a Vietnamese part; the update dialog and the GitHub Release show them.

## 0.2.0

### English

Video All-in-One now starts on a computer where nothing else is installed, and updates are safer.

- FFmpeg and FFprobe are included in the installers for macOS, Windows and Ubuntu. You no longer have to install them yourself. If a bundled tool is damaged, the app tells you to reinstall it.
- Windows: the app opens without a black console window and has its own icon.
- The main window has a minimum size, so the layout is never squeezed out of shape.
- If the app cannot start, for example because its data was made by a newer version, it shows a window that says so and where the data is, instead of closing without a word.
- macOS: one disk image works on both Apple silicon and Intel Macs.
- Windows updates wait until the app has closed, remove the files of the old version that are no longer needed, and put the old version back if the update fails. The app then tells you that the last update did not finish.
- Ubuntu updates install the extra packages that a new version needs.
- If exports are queued or running when you update, the app says how many and lets you update after they finish, update now and stop them, or update later.
- Precise cut picks an H.264 encoder that the bundled FFmpeg offers, and says so plainly when none is available.

Known limits: the first run of an installer still shows a Gatekeeper warning on macOS or a SmartScreen warning on Windows, because the installers are not signed with a paid certificate. The update from 0.1.0 to this version cannot put the old version back if it fails on Windows, because 0.1.0 did not keep a list of its files.

### Tiếng Việt

Video All-in-One nay chạy được trên máy chưa cài gì thêm, và việc cập nhật an toàn hơn.

- Bộ cài cho macOS, Windows và Ubuntu đã kèm sẵn FFmpeg và FFprobe, bạn không cần tự cài nữa. Nếu một công cụ đi kèm bị hỏng, ứng dụng sẽ báo bạn cài lại.
- Windows: ứng dụng mở ra không còn cửa sổ console màu đen và có biểu tượng riêng.
- Cửa sổ chính có kích thước tối thiểu, nên bố cục không bị ép méo.
- Nếu ứng dụng không khởi động được, ví dụ vì dữ liệu do bản mới hơn tạo ra, ứng dụng hiện một cửa sổ nói rõ điều đó và nơi chứa dữ liệu, thay vì đóng lại không một lời.
- macOS: một tệp đĩa dùng được cho cả máy Mac chip Apple và Intel.
- Việc cập nhật trên Windows chờ ứng dụng đóng hẳn, xóa các tệp của bản cũ không còn cần, và khôi phục bản cũ nếu cập nhật lỗi. Khi đó ứng dụng báo rằng lần cập nhật gần nhất chưa hoàn tất.
- Việc cập nhật trên Ubuntu cài thêm các gói mà bản mới cần.
- Nếu đang có việc xuất trong hàng đợi hoặc đang chạy khi bạn cập nhật, ứng dụng cho biết số lượng và cho bạn chọn cập nhật sau khi chúng xong, cập nhật ngay và dừng chúng, hoặc để sau.
- Cắt Chính xác chọn bộ mã hóa H.264 mà FFmpeg đi kèm có, và báo rõ ràng nếu không có bộ nào.

Hạn chế đã biết: lần chạy đầu tiên của bộ cài vẫn hiện cảnh báo Gatekeeper trên macOS hoặc SmartScreen trên Windows, vì bộ cài chưa được ký bằng chứng chỉ trả phí. Việc cập nhật từ 0.1.0 lên bản này không thể khôi phục bản cũ nếu lỗi trên Windows, vì 0.1.0 không giữ danh sách tệp của nó.

## 0.1.0

### English

First release. It works with video files on your computer only; downloading from the web is not part of it yet.

- Add a video file from your computer, see its details and preview it.
- Enter one or more segments by their start and end times and export them, either as separate files or joined into one file. Choose Fast (no re-encoding, cuts snap to the nearest keyframe) or Precise (re-encodes, cuts exactly at the times you set). Your original video is never changed or overwritten.
- Add several videos and press Process all to queue an export for each one.
- Exports run in a job queue that shows each job as queued, running, completed, failed or cancelled. A job that was running when the app closed is shown as failed after the restart.
- Choose English or Vietnamese when you first start the app and change it later in Settings.
- Settings show the app version and let you check for updates.
- The app updates itself: it reads a signed update list from GitHub Releases, downloads the new version, checks its signature and installs it. A broken or unsigned download is rejected and the installed app keeps working.
- FFmpeg and FFprobe must be installed on your computer for now. If they are missing, the app explains how to install them or lets you pick the folder that contains them.

Known limits: the first run of an installer shows a Gatekeeper warning on macOS or a SmartScreen warning on Windows, because the installers are not signed with a paid certificate.

### Tiếng Việt

Bản phát hành đầu tiên. Bản này chỉ làm việc với tệp video trên máy của bạn; tải video từ web chưa có.

- Thêm một tệp video từ máy tính, xem thông tin của tệp và xem trước.
- Nhập một hoặc nhiều đoạn bằng thời điểm bắt đầu và kết thúc, rồi xuất thành các tệp riêng hoặc ghép thành một tệp. Chọn Nhanh (không mã hóa lại, điểm cắt dịch về khung hình chính gần nhất) hoặc Chính xác (mã hóa lại, cắt đúng thời điểm bạn đặt). Video gốc của bạn không bao giờ bị sửa hay ghi đè.
- Thêm nhiều video rồi bấm Xử lý tất cả để đưa việc xuất của từng video vào hàng đợi.
- Việc xuất chạy trong hàng đợi, mỗi việc hiện trạng thái đang chờ, đang chạy, hoàn tất, thất bại hoặc đã hủy. Việc đang chạy lúc ứng dụng đóng sẽ hiện là thất bại sau khi mở lại.
- Chọn tiếng Anh hoặc tiếng Việt khi mở ứng dụng lần đầu và đổi lại sau trong Cài đặt.
- Cài đặt hiển thị phiên bản ứng dụng và cho phép kiểm tra cập nhật.
- Ứng dụng tự cập nhật: đọc danh sách cập nhật có chữ ký từ GitHub Releases, tải bản mới, kiểm tra chữ ký rồi cài đặt. Bản tải về bị hỏng hoặc không có chữ ký sẽ bị từ chối và ứng dụng đang cài vẫn chạy bình thường.
- Hiện tại bạn cần cài sẵn FFmpeg và FFprobe trên máy. Nếu thiếu, ứng dụng sẽ hướng dẫn cách cài hoặc cho bạn chọn thư mục chứa chúng.

Hạn chế đã biết: lần chạy đầu tiên của bộ cài sẽ hiện cảnh báo Gatekeeper trên macOS hoặc SmartScreen trên Windows, vì bộ cài chưa được ký bằng chứng chỉ trả phí.
