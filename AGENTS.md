# Quy ước tổ chức tệp

- Tất cả tệp kiểm thử phải nằm trong thư mục `tests/` của repo.
- Gom script hỗ trợ, công cụ đối chiếu, fixture, dữ liệu mẫu và kết quả kiểm thử
  thủ công vào `tests/` hoặc các thư mục con; không tạo rải rác ở thư mục gốc,
  `src/` hay bên ngoài repo.
- Công cụ hỗ trợ kiểm thử đặt trong `tests/tools/`. Khi di chuyển tệp, cập nhật
  đường dẫn trong script và tài liệu liên quan.
- Các sản phẩm build do Cargo quản lý vẫn dùng thư mục chuẩn `target/`.

# Kiến trúc viet-ime

- Core Rust đặt tại `core-rust/`; thuật toán gõ nằm trong `composition.rs`,
  điều phối sự kiện và output nằm trong `engine.rs`.
- Wrapper tại `wrappers/ibus-c/` và `wrappers/fcitx5-cpp/`, mỗi file plugin dưới
  250 dòng. Wrapper chỉ chuyển sự kiện, metadata/capabilities và thực thi action;
  không thêm luật tiếng Việt hay đọc cấu hình người dùng.
- C ABI chỉ có 5 hàm trong `core-rust/include/viet_core.h`. Chuỗi kết quả do engine
  sở hữu; không thêm hàm giải phóng kết quả. Thay đổi ABI phải cập nhật header,
  hai wrapper và kiểm thử cùng lúc.
- Tất cả kiểm thử vẫn tập trung ở `tests/` tại gốc monorepo. Khai báo đường dẫn
  integration test trong `core-rust/Cargo.toml`, không tạo `core-rust/tests/`.
