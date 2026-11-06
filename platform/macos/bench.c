// Đo bộ gõ qua C ABI (đúng đường SongnguController.m gọi, trừ IPC InputMethodKit): độ trễ mỗi phím, throughput,
// RSS/footprint, CPU khi gõ đều, wakeups lúc rảnh, thời gian nạp.
// build: clang -O2 -Iplatform/macos platform/macos/bench.c target/release/libsongngu.a -o target/bench
// usage: target/bench <Resources dir> <keys> [--marked] [--paced keys_per_sec seconds] [--idle seconds]
// <keys>: `songngu keys <văn bản> [--realistic]` (UTF-8, \b = Backspace, hết dòng = Enter).
#include <libproc.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>
#include "songngu.h"

typedef struct { uint32_t kind, ch; } Ev;

static double ns_per_tick;
static double now_ns(void) { return mach_absolute_time() * ns_per_tick; }

static void mem(const char *label) {
    task_vm_info_data_t vm;
    mach_msg_type_number_t n = TASK_VM_INFO_COUNT;
    task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vm, &n);
    printf("mem %-14s footprint %6.1f MB  resident %6.1f MB\n", label, vm.phys_footprint / 1048576.0, vm.resident_size / 1048576.0);
}

static double cpu_s(void) {
    struct rusage r;
    getrusage(RUSAGE_SELF, &r);
    return r.ru_utime.tv_sec + r.ru_utime.tv_usec / 1e6 + r.ru_stime.tv_sec + r.ru_stime.tv_usec / 1e6;
}

static uint64_t wakeups(void) {
    struct rusage_info_v4 ri;
    if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&ri)) return 0;
    return ri.ri_pkg_idle_wkups + ri.ri_interrupt_wkups;
}

static int cmp(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return x < y ? -1 : x > y;
}

static void report(const char *label, double *ns, size_t n, double wall_s) {
    qsort(ns, n, sizeof *ns, cmp);
    double sum = 0;
    for (size_t i = 0; i < n; i++) sum += ns[i];
#define P(q) ns[(size_t)((n - 1) * (q))]
    printf("%-8s events %zu  avg %.0f ns  p50 %.0f  p95 %.0f  p99 %.0f  p99.9 %.0f  max %.0f ns  throughput %.0f keys/s\n", label, n,
           sum / n, P(0.5), P(0.95), P(0.99), P(0.999), ns[n - 1], wall_s > 0 ? n / wall_s : 0);
}

// Dòng đang gõ (UTF-8), để đưa "chữ trước con trỏ" như app thật: tối đa 64 ký tự cuối.
static char doc[1 << 16];
static size_t doc_len;

static void doc_cut(uint32_t chars) {
    while (chars-- && doc_len) {
        do doc_len--;
        while (doc_len && ((unsigned char)doc[doc_len] & 0xC0) == 0x80);
    }
    doc[doc_len] = 0;
}

static const char *doc_tail(void) {
    size_t i = doc_len;
    for (int c = 0; c < 64 && i > 0; c++) {
        do i--;
        while (i && ((unsigned char)doc[i] & 0xC0) == 0x80);
    }
    return doc + i;
}

static SnAction press(SnHandle *h, Ev e) {
    int wants = e.kind == SN_ESCAPE || e.kind == SN_FLIP || sn_wants_before(h);
    SnAction a = sn_event(h, e.kind, e.ch, wants ? doc_tail() : NULL);
    doc_cut(a.erase + (a.pass && e.kind == SN_BACKSPACE));
    size_t add = strlen(a.commit);
    if (doc_len + add + 1 < sizeof doc) {
        memcpy(doc + doc_len, a.commit, add + 1);
        doc_len += add;
    }
    if (e.kind == SN_OTHER) doc_len = doc[0] = 0;  // Enter: hết tin nhắn
    return a;
}

int main(int argc, char **argv) {
    if (argc < 3) return 1;
    mach_timebase_info_data_t tb;
    mach_timebase_info(&tb);
    ns_per_tick = (double)tb.numer / tb.denom;
    mem("start");

    // đọc chuỗi phím -> sự kiện
    FILE *f = fopen(argv[2], "rb");
    if (!f) return 1;
    size_t cap = 1 << 20, n = 0;
    Ev *evs = malloc(cap * sizeof *evs);
    for (int c; (c = fgetc(f)) != EOF;) {
        if (n + 1 > cap) evs = realloc(evs, (cap *= 2) * sizeof *evs);
        if (c == '\n') { evs[n++] = (Ev){SN_OTHER, 0}; continue; }
        if (c == '\b') { evs[n++] = (Ev){SN_BACKSPACE, 0}; continue; }
        // UTF-8 -> code point
        uint32_t cp = c, more = c >= 0xF0 ? 3 : c >= 0xE0 ? 2 : c >= 0xC0 ? 1 : 0;
        if (more) cp = c & (0x3F >> more);
        while (more--) cp = (cp << 6) | (fgetc(f) & 0x3F);
        evs[n++] = (Ev){SN_CHAR, cp};
    }
    fclose(f);

    // mảng đo của chính harness, cấp và chạm trước để "before load" là mốc trừ ra footprint của bộ gõ
    double *ns = malloc(n * sizeof *ns);
    memset(ns, 0, n * sizeof *ns);
    mem("before load");
    char data[1024], model[1024];
    snprintf(data, sizeof data, "%s/data", argv[1]);
    snprintf(model, sizeof model, "%s/model.bin", argv[1]);
    double t0 = now_ns();
    SnHandle *h = sn_create(data, model, NULL, NULL, NULL);
    double t1 = now_ns();
    if (!h) { fprintf(stderr, "sn_create failed\n"); return 1; }
    printf("startup  sn_create (từ điển + mô hình) %.0f ms\n", (t1 - t0) / 1e6);
    mem("after load");

    double paced_rate = 0, paced_s = 0, idle_s = 0;
    for (int i = 3; i < argc; i++) {
        if (!strcmp(argv[i], "--marked")) sn_option_toggle(h, "marked");
        if (!strcmp(argv[i], "--paced") && i + 2 < argc) { paced_rate = atof(argv[i + 1]); paced_s = atof(argv[i + 2]); }
        if (!strcmp(argv[i], "--idle") && i + 1 < argc) idle_s = atof(argv[i + 1]);
    }
    sn_set_app(h, "com.example.bench");

    // phím đầu tiên sau khi nạp (bộ nhớ đệm CPU còn lạnh)
    double a0 = now_ns();
    press(h, evs[0]);
    printf("startup  first key %.1f us\n", (now_ns() - a0) / 1e3);

    // replay hết corpus, nhanh nhất có thể
    double c0 = cpu_s(), w0 = now_ns();
    for (size_t i = 1; i < n; i++) {
        double s = now_ns();
        press(h, evs[i]);
        ns[i - 1] = now_ns() - s;
    }
    double wall = (now_ns() - w0) / 1e9;
    report("replay", ns, n - 1, wall);
    printf("cpu      replay %.2f s CPU / %.2f s wall\n", cpu_s() - c0, wall);
    mem("after replay");

    if (paced_rate > 0) {
        // gõ đều `paced_rate` phím/giây: %CPU một nhân, wakeups, độ trễ khi bộ nhớ đệm nguội giữa hai phím
        size_t m = (size_t)(paced_rate * paced_s);
        double *pns = malloc(m * sizeof *pns);
        uint64_t step = (uint64_t)(1e9 / paced_rate / ns_per_tick), next = mach_absolute_time();
        double pc0 = cpu_s(), pw0 = now_ns();
        uint64_t wk0 = wakeups();
        for (size_t i = 0; i < m; i++) {
            next += step;
            mach_wait_until(next);
            double s = now_ns();
            press(h, evs[i % n]);
            pns[i] = now_ns() - s;
        }
        double pw = (now_ns() - pw0) / 1e9;
        printf("paced    %.0f keys/s for %.0f s: CPU %.4f%% of one core, %.1f wakeups/s (gồm %.0f lần đánh thức của chính vòng hẹn giờ)\n",
               paced_rate, pw, 100 * (cpu_s() - pc0) / pw, (wakeups() - wk0) / pw, paced_rate);
        report("paced", pns, m, 0);
    }
    if (idle_s > 0) {
        double ic0 = cpu_s();
        uint64_t wk0 = wakeups();
        sleep((unsigned)idle_s);
        printf("idle     %.0f s: CPU %.4f%%, %.2f wakeups/s\n", idle_s, 100 * (cpu_s() - ic0) / idle_s, (wakeups() - wk0) / idle_s);
        mem("after idle");
    }
    return 0;
}
