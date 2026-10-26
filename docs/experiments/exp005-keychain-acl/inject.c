// exp005 注入取证 + 清理：用 DYLD_INSERT_LIBRARIES 注入到「能过两道关的那个程序」（未开强化运行时）里，
// 借它的身份读（只打印长度）或删实验条目。只认服务名 atg.exp005、账户名以 exp 开头。
// 环境变量：EXP005_INJECT_OP=get|del，EXP005_INJECT_ACCOUNT=<exp*>
// 挂在 getppid（小助手 main 第一句）和 SecKeychainSetUserInteractionAllowed（探针 main 第一句）上：
// 构造函数里 Foundation 还没就绪，调钥匙串接口会崩。注入库自己的调用不受挂钩影响。
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static pid_t run(void) {
    const char *op = getenv("EXP005_INJECT_OP");
    const char *a = getenv("EXP005_INJECT_ACCOUNT");
    if (!op || !a || strncmp(a, "exp", 3)) _exit(2);
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    SecKeychainSetUserInteractionAllowed(false);
#pragma clang diagnostic pop
    CFMutableDictionaryRef q = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(q, kSecClass, kSecClassGenericPassword);
    CFDictionarySetValue(q, kSecAttrService, CFSTR("atg.exp005"));
    CFDictionarySetValue(q, kSecAttrAccount, CFStringCreateWithCString(NULL, a, kCFStringEncodingUTF8));
    OSStatus st;
    if (!strcmp(op, "get")) {
        CFDictionarySetValue(q, kSecReturnData, kCFBooleanTrue);
        CFDictionarySetValue(q, kSecMatchLimit, kSecMatchLimitOne);
        CFTypeRef out = NULL;
        st = SecItemCopyMatching(q, &out);
        dprintf(2, "inject: get %s status=%d len=%ld\n", a, (int)st, out ? (long)CFDataGetLength((CFDataRef)out) : -1L);
    } else {
        st = SecItemDelete(q);
        dprintf(2, "inject: del %s status=%d\n", a, (int)st);
    }
    _exit(st == 0 ? 0 : 1);
}

static OSStatus run_probe(Boolean ignored) {
    (void)ignored;
    run();
    return 0;
}

typedef struct { const void *replacement; const void *replacee; } interpose_t;
__attribute__((used, section("__DATA,__interpose"))) static const interpose_t interposers[] = {
    {(const void *)run, (const void *)getppid},
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    {(const void *)run_probe, (const void *)SecKeychainSetUserInteractionAllowed},
#pragma clang diagnostic pop
};
