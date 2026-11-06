// Lọc danh sách tần suất tiếng Anh ("từ số_lần" mỗi dòng): giữ từ a-z, ít nhất 2 chữ, mà bộ kiểm chính tả macOS
// (en_US) chấp nhận ở dạng thường hoặc viết hoa chữ đầu. Danh sách phụ đề lẫn lỗi chính tả và từ ngoại lai.
// build: clang -fobjc-arc -framework AppKit tools/data/en_words.m -o /tmp/en_words
// usage: /tmp/en_words < en_50k.txt > data/en_words.txt
#import <AppKit/AppKit.h>

static BOOL accepted(NSSpellChecker *sc, NSString *w) {
    NSRange r = [sc checkSpellingOfString:w startingAt:0 language:@"en_US" wrap:NO inSpellDocumentWithTag:0 wordCount:NULL];
    return r.location == NSNotFound;
}

int main(void) {
    @autoreleasepool {
        NSSpellChecker *sc = [NSSpellChecker sharedSpellChecker];
        char word[256];
        long long count;
        while (scanf("%255s %lld", word, &count) == 2) {
            size_t n = strlen(word), i = 0;
            while (i < n && word[i] >= 'a' && word[i] <= 'z') i++;
            if (n < 2 || i < n) continue;
            NSString *w = @(word);
            if (accepted(sc, w) || accepted(sc, w.capitalizedString)) printf("%s %lld\n", word, count);
        }
    }
    return 0;
}
