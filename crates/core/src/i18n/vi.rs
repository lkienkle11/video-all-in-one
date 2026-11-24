use super::Text;

/// Vietnamese strings, one arm per text key.
pub fn text(text: Text) -> &'static str {
    match text {
        Text::AppTitle => "Video All-in-One",
        Text::ChooseLanguageTitle => "Chọn ngôn ngữ",
        Text::ChooseLanguageBody => "Chọn ngôn ngữ cho ứng dụng. Bạn có thể đổi sau trong Cài đặt.",
        Text::LanguageEnglish => "English",
        Text::LanguageVietnamese => "Tiếng Việt",
        Text::SettingsTitle => "Cài đặt",
        Text::SettingsLanguage => "Ngôn ngữ",
        Text::SettingsVersion => "Phiên bản {0}",
        Text::UpdateChecking => "Đang kiểm tra cập nhật…",
        Text::UpdateUpToDate => "Bạn đang dùng phiên bản mới nhất",
        Text::UpdateAvailable => "Có phiên bản {0} ({1} MB)",
        Text::UpdateDownload => "Tải xuống",
        Text::UpdateInstall => "Cài đặt",
        Text::UpdateDownloading => "Đang tải {0} trên {1} MB",
        Text::UpdateRetry => "Thử lại",
        Text::UpdateConfirmTitle => "Cập nhật ngay bây giờ?",
        Text::UpdateConfirmBody => {
            "Ứng dụng sẽ khởi động lại để hoàn tất cập nhật. Dung lượng tải: {0} MB."
        }
        Text::UpdateLater => "Để sau",
        Text::UpdateNow => "Cập nhật ngay",
        Text::ErrorGeneric => "Đã xảy ra lỗi. Vui lòng thử lại.",
        Text::SettingsUpdates => "Cập nhật",
        Text::UpdateCheck => "Kiểm tra cập nhật",
        Text::UpdateReady => "Sẵn sàng cài đặt",
        Text::UpdateInstalled => "Đã cài đặt bản cập nhật",
        Text::UpdateFailed => "Không thể cập nhật. Hãy thử lại.",
        Text::UpdateInstallFailed => "Không thể cài đặt bản cập nhật. Hãy thử lại.",
        Text::UpdateDownloaded => "Đã tải {0} MB",
        Text::SettingsSaveFailed => "Không thể lưu ngôn ngữ. Hãy thử lại.",
        Text::Close => "Đóng",
        Text::UpdateDownloadingUnknown => "Đang tải {0} MB",
    }
}
