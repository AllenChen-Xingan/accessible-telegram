// exp005 方案丁「取钥匙小助手」原型：小助手很少重编（cdhash 稳定 → 分区名单一直认它），
// 由它建 / 读钥匙串条目；调用方（每次重编的开发版）把它当子进程起，小助手先核父进程的签名
// 满足要求（atg-dev 证书），过了才把钥匙从管道交出去。
// 用法：keyhelper get <exp*账户>   （只碰服务名 atg.exp005）
// 实验版：不把钥匙写到标准输出，只打印状态与长度；正式版才会把 32 字节写进管道。
// ⚠️ 原型只在开始时按父进程 pid 核一次签名，「核的是谁」和「钥匙交给谁」没绑在一起：
//    自审指出同用户进程不靠注入也能绕过（未实测，见 README「方案丁的已知弱点」）。只证明了机制可行，不是安全实现。
// CALLER_REQ 默认写的是本机 atg-dev 证书的 SHA-1；别的机器由 run.sh 读出证书哈希后 -DCALLER_REQ 传入。
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#ifndef CALLER_REQ
#define CALLER_REQ "certificate leaf = H\"b1c8869ca718e08bbb17f17058d66646f051c245\""
#endif

static CFStringRef S(const char *s) { return CFStringCreateWithCString(NULL, s, kCFStringEncodingUTF8); }

int main(int argc, char **argv) {
    if (argc != 3 || strcmp(argv[1], "get") || strncmp(argv[2], "exp", 3)) return 2;
    pid_t pp = getppid();
    int ppi = (int)pp;
    CFNumberRef pidn = CFNumberCreate(NULL, kCFNumberIntType, &ppi);
    const void *k[] = {kSecGuestAttributePid};
    const void *v[] = {pidn};
    CFDictionaryRef attrs = CFDictionaryCreate(NULL, k, v, 1, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    SecCodeRef code = NULL;
    OSStatus st = SecCodeCopyGuestWithAttributes(NULL, attrs, kSecCSDefaultFlags, &code);
    SecRequirementRef req = NULL;
    if (st == 0) st = SecRequirementCreateWithString(S(CALLER_REQ), kSecCSDefaultFlags, &req);
    if (st == 0) st = SecCodeCheckValidity(code, kSecCSDefaultFlags, req);
    if (st != 0) {
        printf("helper: caller rejected status=%d\n", (int)st);
        return 4;
    }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    SecKeychainSetUserInteractionAllowed(false);
#pragma clang diagnostic pop
    CFMutableDictionaryRef q = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(q, kSecClass, kSecClassGenericPassword);
    CFDictionarySetValue(q, kSecAttrService, S("atg.exp005"));
    CFDictionarySetValue(q, kSecAttrAccount, S(argv[2]));
    CFDictionarySetValue(q, kSecReturnData, kCFBooleanTrue);
    CFDictionarySetValue(q, kSecMatchLimit, kSecMatchLimitOne);
    CFTypeRef out = NULL;
    st = SecItemCopyMatching(q, &out);
    if (st == errSecItemNotFound) {
        UInt8 buf[32];
        if (SecRandomCopyBytes(kSecRandomDefault, sizeof buf, buf) != 0) return 3;
        CFDataRef d = CFDataCreate(NULL, buf, sizeof buf);
        memset(buf, 0, sizeof buf);
        CFDictionaryRemoveValue(q, kSecReturnData);
        CFDictionaryRemoveValue(q, kSecMatchLimit);
        CFDictionarySetValue(q, kSecValueData, d);
        st = SecItemAdd(q, NULL);
        printf("helper: created status=%d\n", (int)st);
        return st == 0 ? 0 : 1;
    }
    printf("helper: get status=%d len=%ld\n", (int)st, out ? (long)CFDataGetLength((CFDataRef)out) : -1L);
    return st == 0 ? 0 : 1;
}
