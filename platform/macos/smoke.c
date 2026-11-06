// Kiểm tra nhanh C ABI với tài nguyên trong bundle, không cần bật bộ gõ.
// build: clang -Iplatform/macos platform/macos/smoke.c target/release/libsongngu.a -o target/smoke
// usage: target/smoke target/Songngu.app/Contents/Resources "phím"   (< Backspace, ~ Esc, ^ Shift+Esc, | Enter)
#include <stdio.h>
#include <string.h>
#include "songngu.h"

int main(int argc, char **argv) {
    if (argc < 3) return 1;
    char data[1024], model[1024];
    snprintf(data, sizeof data, "%s/data", argv[1]);
    snprintf(model, sizeof model, "%s/model.bin", argv[1]);
    SnHandle *h = sn_create(data, model, NULL, NULL, NULL);
    if (!h) { fprintf(stderr, "sn_create failed\n"); return 1; }
    char doc[4096] = "";
    for (const char *p = argv[2]; *p; p++) {
        uint32_t kind = *p == '<' ? SN_BACKSPACE : *p == '~' ? SN_ESCAPE : *p == '^' ? SN_FLIP : *p == '|' ? SN_OTHER : SN_CHAR;
        int wants = kind == SN_ESCAPE || kind == SN_FLIP || sn_wants_before(h);
        SnAction a = sn_event(h, kind, (unsigned char)*p, wants ? doc : NULL);
        // xoá `cut` ký tự cuối
        uint32_t cut = a.erase + (a.pass && kind == SN_BACKSPACE);
        size_t n = strlen(doc);
        for (uint32_t i = 0; i < cut && n > 0; i++) {
            do { n--; } while (n > 0 && ((unsigned char)doc[n] & 0xC0) == 0x80);
        }
        doc[n] = 0;
        strncat(doc, a.commit, sizeof doc - strlen(doc) - 1);
        printf("%c  app=[%s]%s%s\n", *p, doc, a.erase ? "  replace" : "", a.pass ? "  pass" : "");
    }
    return 0;
}
