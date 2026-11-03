# Mục tiêu hỗ trợ Linux

Một core Rust, hai wrapper IBus/Fcitx5, một giao diện cấu hình chung. Người dùng
cuối cài binary đã build; không cần Rust/compiler. Windows là máy build qua
toolchain cross-compile, không cài/chạy VM, WSL hoặc Linux tạm.

Không có một package nhị phân độc lập dùng cho mọi Linux: framework cần plugin
đúng ABI, CPU và libc. Phát hành theo ma trận; dùng chung binary khi đã kiểm tra
ABI/dependency, không tách theo tên distro nếu không cần thiết.

| Nhóm đích | Định dạng | Trạng thái |
|---|---|---|
| Debian 12 + Deepin 25, amd64, glibc | Một DEB chung | Đang hoàn tất đối chiếu SDK hai nền; chưa chạy desktop thật |
| ARM64, glibc | DEB/RPM theo ABI | Có tham số target; chưa build/kiểm chứng |
| Fedora/openSUSE | RPM theo ABI | Chưa hoàn tất đường cross-build Windows |
| Arch/Manjaro | pkg.tar.zst | Chưa triển khai |
| Alpine, musl | APK | Chưa triển khai cross-build/package |
| NixOS/Guix, hệ immutable | Native/declarative packaging | Chưa triển khai |
| Framework trong sandbox Flatpak | Extension tương ứng runtime | Chưa triển khai |
| LoongArch/RISC-V, các CPU khác | Gói theo target | Chưa triển khai |

AppImage hoặc tarball không tự làm cho plugin được host IBus/Fcitx5 nạp trên mọi
distro. Không bundle glibc/libstdc++/framework khác phiên bản vào tiến trình host.
Ứng dụng controller có thể phân phối khác plugin, nhưng dùng cùng protocol/config.

Mỗi nền cần kiểm: binary đúng CPU/libc; dependency từ kho đích; SONAME/RPATH;
C ABI; lifecycle focus/reset; install/uninstall; nhận diện input method; GUI/IPC;
gõ thực tế trên X11/Wayland, GTK/Qt/Electron. Chỉ kiểm ELF/symbol không đủ chứng
minh runtime hoạt động. Ma trận này là phạm vi phát triển, không phải tuyên bố
đã hỗ trợ tất cả nền liệt kê.
