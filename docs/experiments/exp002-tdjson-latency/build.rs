// tdlib-rs 自己的 build.rs 设的 rpath 不会传给依赖它的包；按 tdlib-rs README 的用法，
// 在本包再调一次它的构建助手：下载预编译 TDLib 1.8.61 到本包 OUT_DIR，并给本包的可执行文件写 rpath。
fn main() {
    tdlib_rs::build::build(None);
}
