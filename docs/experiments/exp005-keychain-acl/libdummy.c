// exp005 方案乙：替身动态库，模拟 libtdjson.dylib，验证强化运行时下的库加载方式（rpath + 同团队签名）
int exp005_dummy(void) { return 42; }
