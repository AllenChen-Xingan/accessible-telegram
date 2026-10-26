// exp006 探针（#38 迁移演练）：只碰服务名 atg.exp006、账户名以 exp 开头的实验条目。只打印状态码与长度，绝不打印值。
// 用法：probe add|get|getui|del <exp*-账户名>
//   add/get/del 关交互：本该问人的情形不弹框、直接失败（本机实测 -25293）
//   getui       开着交互取值：过不了关就弹框（迁移时人点「始终允许」那一步）
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <stdio.h>
#include <string.h>
#ifndef NONCE
#define NONCE "0"
#endif
static CFStringRef S(const char *s) { return CFStringCreateWithCString(NULL, s, kCFStringEncodingUTF8); }
int main(int argc, char **argv) {
    if (argc != 3 || strncmp(argv[2], "exp", 3) != 0) { fprintf(stderr, "usage\n"); return 2; }
    const char *op = argv[1];
    printf("nonce=%s op=%s ", NONCE, op);
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    if (strcmp(op, "getui")) SecKeychainSetUserInteractionAllowed(false);
#pragma clang diagnostic pop
    CFMutableDictionaryRef q = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(q, kSecClass, kSecClassGenericPassword);
    CFDictionarySetValue(q, kSecAttrService, S("atg.exp006"));
    CFDictionarySetValue(q, kSecAttrAccount, S(argv[2]));
    OSStatus st;
    if (!strcmp(op, "add")) {
        UInt8 buf[32];
        if (SecRandomCopyBytes(kSecRandomDefault, sizeof buf, buf) != 0) return 3;
        CFDataRef d = CFDataCreate(NULL, buf, sizeof buf);
        memset(buf, 0, sizeof buf);
        CFDictionarySetValue(q, kSecValueData, d);
        st = SecItemAdd(q, NULL);
        printf("status=%d\n", (int)st);
    } else if (!strcmp(op, "get") || !strcmp(op, "getui")) {
        CFDictionarySetValue(q, kSecReturnData, kCFBooleanTrue);
        CFDictionarySetValue(q, kSecMatchLimit, kSecMatchLimitOne);
        CFTypeRef out = NULL;
        st = SecItemCopyMatching(q, &out);
        printf("status=%d len=%ld\n", (int)st, out ? (long)CFDataGetLength((CFDataRef)out) : -1L);
    } else if (!strcmp(op, "del")) {
        st = SecItemDelete(q);
        printf("status=%d\n", (int)st);
    } else { fprintf(stderr, "bad op\n"); return 2; }
    return st == 0 ? 0 : 1;
}
