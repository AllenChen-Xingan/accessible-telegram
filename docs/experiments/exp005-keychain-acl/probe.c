// exp005 探针：只碰服务名 atg.exp005 的实验条目（账户名由参数给，须以 exp 开头）。
// 用法：probe add|get|del|addnp|addacc|adddp|viahelper <账户名>
//   add       SecItemAdd（与 security-framework 的 set_generic_password 同路：传统文件钥匙串）
//   get       SecItemCopyMatching 取值。所有操作开头都关掉交互（SecKeychainSetUserInteractionAllowed(false)）：
//             本该问人的情形不弹框，直接失败。本机 macOS 27.0.1 实测返回的是 -25293（errSecAuthFailed），
//             不是文档里常见的 -25308（errSecInteractionNotAllowed）
//   del       SecItemDelete
//   addnp     建条目后，建它的程序自己去掉分区名单那条 ACL
//   addacc    建条目时带自建的访问对象（信任名单 = 自己）
//   adddp     往数据保护钥匙串加（kSecUseDataProtectionKeychain=true）
//   viahelper 自己不碰钥匙串，起 EXP005_HELPER 指的小助手当子进程去取（方案丁）
// 只打印状态码与取到的字节数，绝不打印值。
// NONCE 在编译时 -DNONCE=... 注入，用来模拟「重编译后 cdhash 变了」。
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <stdio.h>
#include <string.h>
#include <spawn.h>
#include <stdlib.h>
#include <sys/wait.h>
extern char **environ;

#ifndef NONCE
#define NONCE "0"
#endif

static const char *SERVICE = "atg.exp005";

static CFStringRef S(const char *s) { return CFStringCreateWithCString(NULL, s, kCFStringEncodingUTF8); }

int main(int argc, char **argv) {
    if (argc != 3 || strncmp(argv[2], "exp", 3) != 0) {
        fprintf(stderr, "usage: probe add|get|del|addnp|addacc|adddp|viahelper <exp*-account>\n");
        return 2;
    }
    const char *op = argv[1];
    printf("nonce=%s op=%s account=%s ", NONCE, op, argv[2]);

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    SecKeychainSetUserInteractionAllowed(false);
#pragma clang diagnostic pop

    CFMutableDictionaryRef q = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(q, kSecClass, kSecClassGenericPassword);
    CFDictionarySetValue(q, kSecAttrService, S(SERVICE));
    CFDictionarySetValue(q, kSecAttrAccount, S(argv[2]));

    OSStatus st;
    if (!strcmp(op, "add") || !strcmp(op, "adddp")) {
        UInt8 buf[32];
        if (SecRandomCopyBytes(kSecRandomDefault, sizeof buf, buf) != 0) return 3;
        CFDataRef d = CFDataCreate(NULL, buf, sizeof buf);
        memset(buf, 0, sizeof buf);
        CFDictionarySetValue(q, kSecValueData, d);
        if (!strcmp(op, "adddp")) CFDictionarySetValue(q, kSecUseDataProtectionKeychain, kCFBooleanTrue);
        st = SecItemAdd(q, NULL);
        printf("status=%d\n", (int)st);
    } else if (!strcmp(op, "addnp") || !strcmp(op, "addacc")) {
        // addnp：建条目后，建它的程序自己去掉分区名单那条 ACL（不许交互）
        // addacc：建条目时带自建的 SecAccess（信任名单 = 自己），看分区名单还加不加
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        UInt8 buf[32];
        if (SecRandomCopyBytes(kSecRandomDefault, sizeof buf, buf) != 0) return 3;
        CFDataRef d = CFDataCreate(NULL, buf, sizeof buf);
        memset(buf, 0, sizeof buf);
        CFDictionarySetValue(q, kSecValueData, d);
        if (!strcmp(op, "addacc")) {
            SecTrustedApplicationRef me = NULL;
            OSStatus s0 = SecTrustedApplicationCreateFromPath(NULL, &me);
            const void *apps[] = {me};
            CFArrayRef list = CFArrayCreate(NULL, apps, 1, &kCFTypeArrayCallBacks);
            SecAccessRef acc = NULL;
            OSStatus s1 = SecAccessCreate(S(SERVICE), list, &acc);
            printf("trusted=%d access=%d ", (int)s0, (int)s1);
            CFDictionarySetValue(q, kSecAttrAccess, acc);
            st = SecItemAdd(q, NULL);
            printf("status=%d\n", (int)st);
        } else {
            CFDictionarySetValue(q, kSecReturnRef, kCFBooleanTrue);
            CFTypeRef ref = NULL;
            st = SecItemAdd(q, &ref);
            printf("add=%d ", (int)st);
            if (st != 0 || !ref) { printf("\n"); return 1; }
            SecAccessRef acc = NULL;
            st = SecKeychainItemCopyAccess((SecKeychainItemRef)ref, &acc);
            printf("copyaccess=%d ", (int)st);
            if (st != 0) { printf("\n"); return 1; }
            CFArrayRef acls = NULL;
            SecAccessCopyACLList(acc, &acls);
            int removed = 0;
            for (CFIndex i = 0; acls && i < CFArrayGetCount(acls); i++) {
                SecACLRef acl = (SecACLRef)CFArrayGetValueAtIndex(acls, i);
                CFArrayRef auths = SecACLCopyAuthorizations(acl);
                if (auths && CFArrayContainsValue(auths, CFRangeMake(0, CFArrayGetCount(auths)), kSecACLAuthorizationPartitionID)) {
                    OSStatus r = SecACLRemove(acl);
                    printf("aclremove=%d ", (int)r);
                    if (r == 0) removed++;
                }
            }
            st = SecKeychainItemSetAccess((SecKeychainItemRef)ref, acc);
            printf("removed=%d setaccess=%d\n", removed, (int)st);
        }
#pragma clang diagnostic pop
    } else if (!strcmp(op, "viahelper")) {
        // 方案丁：自己不碰钥匙串，起小助手（路径取 EXP005_HELPER）当子进程去取
        const char *h = getenv("EXP005_HELPER");
        if (!h) return 2;
        printf("\n");
        fflush(stdout);
        char *args[] = {(char *)h, "get", argv[2], NULL};
        pid_t pid;
        int rc = posix_spawn(&pid, h, NULL, NULL, args, environ);
        int ws = 0;
        if (rc == 0) waitpid(pid, &ws, 0);
        printf("spawn=%d helper_exit=%d\n", rc, WIFEXITED(ws) ? WEXITSTATUS(ws) : -1);
        return (rc == 0 && WIFEXITED(ws) && WEXITSTATUS(ws) == 0) ? 0 : 1;
    } else if (!strcmp(op, "get")) {
        CFDictionarySetValue(q, kSecReturnData, kCFBooleanTrue);
        CFDictionarySetValue(q, kSecMatchLimit, kSecMatchLimitOne);
        CFTypeRef out = NULL;
        st = SecItemCopyMatching(q, &out);
        long n = out ? (long)CFDataGetLength((CFDataRef)out) : -1;
        printf("status=%d len=%ld\n", (int)st, n);
    } else if (!strcmp(op, "del")) {
        st = SecItemDelete(q);
        printf("status=%d\n", (int)st);
    } else {
        fprintf(stderr, "bad op\n");
        return 2;
    }
    return st == 0 ? 0 : 1;
}
