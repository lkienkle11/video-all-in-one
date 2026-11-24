# Changelog

All notable changes to Video All-in-One are written here, newest first. Every version has an
English part and a Vietnamese part; the update dialog and the GitHub Release show them.

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
