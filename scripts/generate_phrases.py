import os
import glob
import re

BPMF_MAP = {
    'ㄅ': '1', 'ㄆ': 'q', 'ㄇ': 'a', 'ㄈ': 'z',
    'ㄉ': '2', 'ㄊ': 'w', 'ㄋ': 's', 'ㄌ': 'x',
    'ㄍ': 'e', 'ㄎ': 'd', 'ㄏ': 'c',
    'ㄐ': 'r', 'ㄑ': 'f', 'ㄒ': 'v',
    'ㄓ': '5', 'ㄔ': 't', 'ㄕ': 'g', 'ㄖ': 'b',
    'ㄗ': 'y', 'ㄘ': 'h', 'ㄙ': 'n',
    'ㄧ': 'u', 'ㄨ': 'j', 'ㄩ': 'm',
    'ㄚ': '8', 'ㄛ': 'i', 'ㄜ': 'k', 'ㄝ': ',',
    'ㄞ': '9', 'ㄟ': 'o', 'ㄠ': 'l', 'ㄡ': '.',
    'ㄢ': '0', 'ㄣ': 'p', 'ㄤ': ';', 'ㄥ': '/',
    'ㄦ': '-',
    'ˊ': '6', 'ˇ': '3', 'ˋ': '4', '˙': '7',
}

def syllable_to_code(syl):
    return ''.join(BPMF_MAP.get(c, '') for c in syl)

def bpmf_to_codes(bpmf_str):
    # Splits by space or comma
    tokens = [t.strip() for t in re.split(r'[\s,]+', bpmf_str) if t.strip()]
    if not tokens:
        return None, None
    syl_codes = [syllable_to_code(t) for t in tokens if syllable_to_code(t)]
    if not syl_codes:
        return None, None
    full_code = ''.join(syl_codes)
    first_code = syl_codes[0]
    return full_code, first_code

entries = [] # list of (code, phrase)

# 1. Parse polobread KeyKey TSVs in data/phrases
tsv_dir = os.path.join(os.path.dirname(__file__), '..', 'data', 'phrases')
for tsv_path in glob.glob(os.path.join(tsv_dir, '*.tsv')):
    with open(tsv_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        for idx, line in enumerate(lines):
            line = line.strip()
            if not line or idx == 0:
                continue
            parts = line.split('\t')
            if len(parts) >= 3:
                phrase = parts[0].strip()
                bpmf = parts[2].strip()
                full_code, first_code = bpmf_to_codes(bpmf)
                if full_code and phrase:
                    entries.append((full_code, phrase))
                    if first_code and first_code != full_code:
                        entries.append((first_code, phrase))

# 2. Parse YahooKeyKey Overrides/Keke-AddBpmf.txt
keke_path = os.path.join(os.path.dirname(__file__), '..', 'YahooKeyKey-Source-1.1.2528', 'Distributions', 'Takao', 'DataSource', 'Overrides', 'Keke-AddBpmf.txt')
if os.path.exists(keke_path):
    with open(keke_path, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            line = line.strip()
            if line.startswith('+bpmf '):
                parts = line[6:].strip().split()
                if len(parts) >= 2:
                    phrase = parts[0].strip()
                    bpmf = parts[1].strip()
                    full_code, first_code = bpmf_to_codes(bpmf)
                    if full_code and phrase:
                        entries.append((full_code, phrase))
                        if first_code and first_code != full_code:
                            entries.append((first_code, phrase))

# Deduplicate preserving order
seen = set()
unique_entries = []
for code, phrase in entries:
    if (code, phrase) not in seen:
        seen.add((code, phrase))
        unique_entries.append((code, phrase))

print(f"Total compiled phrase entries: {len(unique_entries)}")

out_dir = os.path.join(os.path.dirname(__file__), '..', 'crates', 'keykey-core', 'data')
os.makedirs(out_dir, exist_ok=True)
out_cin = os.path.join(out_dir, 'phrases.cin')

with open(out_cin, 'w', encoding='utf-8') as f:
    f.write("%gen_inp\n")
    f.write("%ename KeyKey Modern Phrases\n")
    f.write("%cname 常用詞庫\n")
    f.write("%selkey 123456789\n")
    f.write("%chardef begin\n")
    for code, phrase in unique_entries:
        f.write(f"{code}\t{phrase}\n")
    f.write("%chardef end\n")

print(f"Written to {out_cin} successfully.")
