"""Emit the public, deterministic trial archive. Reads only the original memory."""
import argparse
import hashlib
import json
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BASE = ROOT.parent / "trials"
SEED = "loam-workshop-archive-v1"
# Source memory ID, same-action sibling, handling step for a rehearsal.
RULES = [
    ("m001", "雫形の蜜菓子", "見本を菱形の紙に置く"),
    ("m002", "three-cup infusion set", "tuck a leaf-shaped marker beneath the cover"),
    ("m003", "魚尾の木型", "輪郭を方眼の厚紙になぞる"),
    ("m004", "quince cordial", "place a grey coaster under the test cup"),
    ("m005", "丘便", "提げ手に琥珀の飾り環を掛ける"),
    ("m006", "honey lattice", "leave a folded felt mat under the frame"),
    ("m037", "持ち帰りの角箱", "胴に橙色の紙片を差す"),
    ("m038", "garden flasks", "tie a short linen loop around the neck"),
    ("m039", "配達便の花台", "持ち手の間に折布を入れる"),
    ("m040", "enamel platters", "place a cork disc between layers"),
    ("m041", "展示用の星形パン", "中心に紙の花を立てる"),
    ("m042", "rental barrows", "hang the check card on the left handle"),
]
DECISIONS = [
    ("m007", ["薄緑", "麦色", "銅色", "白", "深紅", "灰色"], "{}の帯"),
    ("m008", ["third", "first", "fourth", "fifth", "sixth", "seventh"], "open at the {} bell"),
    ("m009", ["二列", "円形", "十字", "四角", "斜め一列", "平行三列"], "{}の机配置"),
    ("m010", ["Thistle Tap", "Cloud Knot", "Mist Drum", "Soft Thrum", "Twig Song", "Honey Spin"], 'print “{}”'),
    ("m011", ["北戸", "脇庭", "西門", "表庭", "東戸", "中庭"], "{}で受取"),
    ("m012", ["four", "five", "six", "seven", "eight", "ten"], "{} crescents per carton"),
]
# Each subject is a distinct physical object or inspection, not another assertion about the original.
NEARBY = [
    ("m013", "水玉の押し板", "黄箱に納められたらしい"),
    ("m015", "reed gauge carrying case", "may be in the green cupboard"),
    ("m017", "二段窯の長盆", "入口を縦向きで通れそうだ"),
    ("m019", "trellis border offcut", "may be in the nut-brown folder"),
    ("m021", "引き出しの浅板", "裏口から通せそうだ"),
    ("m023", "ink wheel spare handle", "may be in the tin marked Fog"),
    ("m043", "茶釉の試験片", "乾いているらしい"),
    ("m044", "folding stand foot", "might have been re-glued"),
    ("m045", "砂糖輪の試食片", "届いたらしい"),
    ("m046", "lantern timer display sleeve", "may have a four-hour scale"),
    ("m047", "戸袋の金具", "取り替えられたらしい"),
    ("m048", "printed dial backing card", "might resist a light splash"),
]
WORK = [
    ("m025", "小袋と大袋を検品した"),
    ("m027", "east and west crates were counted"),
    ("m029", "両日分の焼き札を印刷した"),
    ("m031", "both cabinets of leaf moulds were polished"),
    ("m033", "笛箱の仕切りを数えた"),
    ("m035", "both faces of the wall menu were proofread"),
]
ROOMS = ["籾殻室", "煉瓦廊", "薄荷室", "南風庫", "波裏室", "灰釉間",
         "cinder alcove", "saffron loft", "mallow room", "birch gallery",
         "thimble attic", "flint nook", "pebble bay"]
# 13 rooms × 15 objects × 3 editions × 4 entry types. Select bilingual names by room.
OBJECTS = [
    ("木端見本", "woodcut sample"), ("蜜蝋片", "wax tablet"), ("房飾り", "fringe tassel"),
    ("革輪", "leather ring"), ("染見本", "dye swatch"), ("針山", "pin cushion"),
    ("星型錘", "star weight"), ("織片", "woven patch"), ("丸刷毛", "round brush"),
    ("小鏝", "tiny trowel"), ("竹枠", "bamboo frame"), ("釉小皿", "glaze saucer"),
    ("紙綴具", "paper clasp"), ("陶製小鈴", "clay bell"), ("炭筆筒", "charcoal tube"),
]


def digest(value):
    return hashlib.sha256((SEED + "|" + value).encode()).hexdigest()


def japanese(text):
    return any(ord(c) > 127 for c in text)


def encode(obj):
    return json.dumps(obj, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()


def opaque(key):
    return "w-" + digest("handle:" + key)[:16]


def extras(base):
    for src, sibling, step in RULES:
        source = base[src]
        for idx in range(1, 13):
            exercise = f"教材第{idx}巻" if japanese(sibling) else f"lesson folio {idx}"
            for family, target in (("rehearsal", source["target"]), ("sibling", sibling)):
                name = f"{family}-{src}-{idx}"
                condition = ("講習用の模擬作業／" + exercise if japanese(sibling)
                             else "classroom simulation / " + exercise)
                text = (f"{condition}で{target}を扱う時だけ、{step}。実用品には適用しない。"
                        if japanese(sibling) else
                        f"For {target} only in {condition}, {step}. Not for ordinary workshop stock.")
                yield name, family, dict(kind="rule", target=target, action=source["action"],
                    scene=condition, condition="simulation-only", exceptions=["ordinary operations"], text=text)

    for src, options, pattern in DECISIONS:
        for idx, option in enumerate(options):
            key = f"revision-{src}-{idx}"
            at = f"2041-{idx + 1:02}-03T10:00:00Z"
            target = base[src]["target"]
            text = (f"{at[:7]} 当時の{target}の決定は「{pattern.format(option)}」。後の改訂で失効。"
                    if japanese(target) else
                    f"The {at[:7]} decision for {target} was to {pattern.format(option)}. Superseded.")
            successor = f"revision-{src}-{idx+1}" if idx < len(options)-1 else src
            yield key, "revision", dict(kind="decision", target=target, text=text,
                state="superseded", speech="directive", at=at, successor=successor)

    for src, item, claim in NEARBY:
        for idx in range(1, 19):
            target = f"{item}・試作{idx}号" if japanese(item) else f"{item}, prototype {idx}"
            text = (f"担当が「{target}は{claim}」と言っていた。この試作品の話だけで現物確認はまだ。"
                    if japanese(item) else
                    f"A helper says the {target} {claim}. No inspection of this prototype is recorded.")
            yield f"report-{src}-{idx}", "report", dict(kind="fact", target=target,
                certainty="reported", source_role="sample attendant", text=text)

    for src, result in WORK:
        target = base[src]["target"]
        for idx in range(20):
            day = date(2041, 1, 5) + timedelta(days=idx * 14)
            period = day.isoformat()
            text = (f"{period} の旧ロットだけで、{target}は{result}。この検数は別の便に繰り越さない。"
                    if japanese(target) else
                    f"For {target}, {result} for the closed lot dated {period} only. Later lots have separate slips.")
            yield f"lot-{src}-{idx}", "closed-lot", dict(kind="experience", target=target,
                period=period, at=period+"T09:00:00Z", text=text)

    for n, room in enumerate(ROOMS):
        ja = japanese(room)
        for j, pair in enumerate(OBJECTS):
            for edition in range(1, 4):
                target = (f"{room}の{pair[0]}・第{edition}組" if ja else
                          f"{pair[1]} set {edition} in the {room}")
                shade = (["薄荷色", "朱色", "亜麻色"] if ja else ["mint", "vermilion", "flax"])[edition-1]
                moment = f"2042-03-{1+(n+j+edition)%8:02}T05:00:00Z"
                for kind, statement, attrs in [
                    ("rule", f"{target}の片付けには{shade}の布を敷く。" if ja else
                     f"Lay a {shade} cloth beneath the {target} before shelving.",
                     dict(scene="after use", action="shelve", exceptions=[])),
                    ("decision", f"{target}の貸出票の縁は{shade}に決まった。" if ja else
                     f"The checkout slip for the {target} will have a {shade} border.",
                     dict(state="decided", speech="directive", at=moment)),
                    ("fact", f"{target}の袋口は縫い直したと当番から聞いた。" if ja else
                     f"The attendant says the pouch for the {target} has a new seam.",
                     dict(certainty="reported", source_role="room attendant")),
                    ("experience", f"{target}用の台紙を裁ち、寸法票に収まりを記録した。" if ja else
                     f"We cut a mount for the {target} and entered its fit on a sizing slip.",
                     dict(at=moment)),
                ]:
                    yield f"room-{n}-{j}-{edition}-{kind}", "bulk", dict(kind=kind,target=target,text=statement,**attrs)


def build():
    originals = json.loads((BASE / "memory.json").read_text(encoding="utf-8"))
    base = {r["id"]:r for r in originals}
    catalog = [(r["id"], "original", dict(r)) for r in originals] + list(extras(base))
    handles = {key:opaque(key) for key, _, _ in catalog}
    records = []
    for key, family, data in catalog:
        r = dict(data, id=handles[key])
        for field in ("successor", "evidence"):
            if field in r:
                r[field] = handles[r[field]]
        records.append((key, family, r))
    # Filing sequence is independent of semantic event time and of trial prompts.
    return sorted(records, key=lambda entry:digest("shelf:"+entry[0]))


def archive_bytes(records):
    return b"".join(encode(r)+b"\n" for _, _, r in records)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate the frozen workshop ledger")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_bytes(archive_bytes(build()))
