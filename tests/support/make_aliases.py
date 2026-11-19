"""Generate alternatives offline from one registration, using WordNet 3.0 and OMW 1.4."""

import json
import re
import sys
import unicodedata

from nltk.corpus import wordnet as wn

record = json.load(sys.stdin)
text = unicodedata.normalize(
    "NFKC", " ".join((record["text"], record["scene"]))
).lower()
terms = {(w, "eng") for w in re.findall(r"[a-z]+", text) if len(w) > 2}
for run in re.findall(r"[ぁ-ヿ一-龯]+", text):
    for start in range(len(run)):
        for size in range(2, min(7, len(run) - start + 1)):
            terms.add((run[start : start + size], "jpn"))
alternatives = set()
for term, lang in sorted(terms):
    for synset in wn.synsets(term, lang=lang)[:2]:
        for language in ("eng", "jpn"):
            alternatives.update(
                w.replace("_", " ") for w in synset.lemma_names(language)[:4]
            )
print(
    json.dumps(
        sorted(w for w in alternatives if w.lower() not in text), ensure_ascii=False
    )
)
