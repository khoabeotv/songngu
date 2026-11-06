// Vẽ biểu tượng bộ gõ: khung bo góc 20×16 pt phủ kín ảnh, chữ "SN" khoét rỗng (đúng tỉ lệ nhãn "A" của ABC trong menu
// nguồn nhập; ảnh vuông có lề bị co nhỏ hơn các nhãn khác). Vector PDF, chữ là đường viền glyph thật nên khoét được
// trong PDF. Info.plist đánh dấu template (TISIconIsTemplate): hệ thống tô theo giao diện sáng/tối.
// usage: mkicon out.pdf [preview.png]
#import <Cocoa/Cocoa.h>
#import <CoreText/CoreText.h>

static const CGFloat W = 20, H = 16;

// Khung trừ chữ. Glyph của font hệ thống có nét chồng nhau (nét chéo chữ N): gộp chúng trước rồi mới trừ, tô
// even-odd thì chỗ chồng bị thủng. Gốc toạ độ góc dưới trái, đơn vị pt.
static CGPathRef iconPath(void) {
    CGMutablePathRef p = CGPathCreateMutable();
    NSFont *font = [NSFont systemFontOfSize:10.5 weight:NSFontWeightSemibold];
    NSAttributedString *s = [[NSAttributedString alloc] initWithString:@"SN"
                                                            attributes:@{NSFontAttributeName : font, NSKernAttributeName : @0.3}];
    CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)s);
    // canh giữa theo hình chữ thật, không theo hộp dòng
    CGRect b = CTLineGetBoundsWithOptions(line, kCTLineBoundsUseGlyphPathBounds);
    CGFloat ox = (W - b.size.width) / 2 - b.origin.x, oy = (H - b.size.height) / 2 - b.origin.y;
    for (id r in (__bridge NSArray *)CTLineGetGlyphRuns(line)) {
        CTRunRef run = (__bridge CTRunRef)r;
        CTFontRef f = CFDictionaryGetValue(CTRunGetAttributes(run), kCTFontAttributeName);
        CFIndex n = CTRunGetGlyphCount(run);
        CGGlyph glyphs[n];
        CGPoint pos[n];
        CTRunGetGlyphs(run, CFRangeMake(0, n), glyphs);
        CTRunGetPositions(run, CFRangeMake(0, n), pos);
        for (CFIndex i = 0; i < n; i++) {
            CGAffineTransform t = CGAffineTransformMakeTranslation(ox + pos[i].x, oy + pos[i].y);
            CGPathRef g = CTFontCreatePathForGlyph(f, glyphs[i], &t);
            if (g) CGPathAddPath(p, NULL, g);
            if (g) CGPathRelease(g);
        }
    }
    CFRelease(line);
    CGPathRef text = CGPathCreateCopyByNormalizing(p, false);
    CGPathRef box = CGPathCreateWithRoundedRect(CGRectMake(0, 0, W, H), 3.6, 3.6, NULL);
    CGPathRef out = CGPathCreateCopyBySubtractingPath(box, text, false);
    CGPathRelease(p);
    CGPathRelease(text);
    CGPathRelease(box);
    return out;
}

static void fillIcon(CGContextRef c, CGRect r, CGColorRef color) {
    CGContextSaveGState(c);
    CGContextTranslateCTM(c, r.origin.x, r.origin.y);
    CGContextScaleCTM(c, r.size.width / W, r.size.height / H);
    CGPathRef p = iconPath();
    CGContextAddPath(c, p);
    CGContextSetFillColorWithColor(c, color);
    CGContextFillPath(c);
    CGPathRelease(p);
    CGContextRestoreGState(c);
}

static BOOL writePDF(NSString *path) {
    CGRect box = CGRectMake(0, 0, W, H);
    CGContextRef c = CGPDFContextCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:path], &box, NULL);
    if (!c) return NO;
    CGPDFContextBeginPage(c, NULL);
    fillIcon(c, box, CGColorGetConstantColor(kCGColorBlack));
    CGPDFContextEndPage(c);
    CGPDFContextClose(c);
    CGContextRelease(c);
    return YES;
}

// Xem trước: nền tối (trắng) và sáng (đen), cỡ to và cỡ thật ở 2x phóng bằng điểm ảnh.
static BOOL writePreview(NSString *path) {
    size_t w = 560, h = 300;
    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    CGContextRef c = CGBitmapContextCreate(NULL, w, h, 8, 0, cs, kCGImageAlphaPremultipliedLast);
    for (int i = 0; i < 2; i++) {
        CGFloat x = 280 * i;
        CGFloat bg = i ? 0.93 : 0.15, fg = i ? 0.1 : 1;
        CGContextSetRGBFillColor(c, bg, bg, bg, 1);
        CGContextFillRect(c, CGRectMake(x, 0, 280, h));
        CGColorRef col = CGColorCreateGenericGray(fg, 1);
        fillIcon(c, CGRectMake(x + 40, 110, 200, 160), col);
        // cỡ thật: 40×32 điểm ảnh, phóng 2 lần
        CGContextRef small = CGBitmapContextCreate(NULL, 40, 32, 8, 0, cs, kCGImageAlphaPremultipliedLast);
        fillIcon(small, CGRectMake(0, 0, 40, 32), col);
        CGImageRef img = CGBitmapContextCreateImage(small);
        CGContextSetInterpolationQuality(c, kCGInterpolationNone);
        CGContextDrawImage(c, CGRectMake(x + 100, 20, 80, 64), img);
        CGImageRelease(img);
        CGContextRelease(small);
        CGColorRelease(col);
    }
    CGImageRef out = CGBitmapContextCreateImage(c);
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:out];
    CGImageRelease(out);
    CGContextRelease(c);
    CGColorSpaceRelease(cs);
    return [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:path atomically:YES];
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc < 2) return 1;
        if (!writePDF([NSString stringWithUTF8String:argv[1]])) return 1;
        if (argc > 2 && !writePreview([NSString stringWithUTF8String:argv[2]])) return 1;
    }
    return 0;
}
