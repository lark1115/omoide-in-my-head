"""Generate a second seeded workshop snapshot without consulting tasks or answers."""
import argparse
import hashlib
import json
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SEED = "kiln-notebook-v2"

# Same operation applied to distinct goods. Each bounded instruction concerns a
# labelled scale model used for a fitting class, never production stock.
SIBLINGS = [
    "氷柱形の砂菓子", "twist roll cushion", "弓形の木型", "ginger cordial vial",
    "胡桃箱の納入口", "saffron flower sleeve", "展示用の藍瓶", "storm crate cover",
    "飾り用の糖枝", "borrowed portrait stand", "貸出用の笛箱", "inked spice plaques",
]
FORMER = [
    ["朝露", "風音", "花舟", "赤土", "薄日", "野道"],
    ["07:15", "07:45", "08:10", "09:20", "08:05", "09:10"],
    ["白門", "南の小路", "納屋", "奥の腰掛け", "橋のたもと", "土間"],
    ["four", "five", "six", "eight", "nine", "ten"],
    ["三角", "貝殻形", "八角", "舟形", "半円", "楕円"],
    ["plain ochre", "blue rings", "lime dots", "crosshatch", "grey checks", "copper stars"],
]
PROTOTYPES = [
    ("菱穴の抜き具の把手", "小箱に移されたらしい"),
    ("icing compass pouch", "may have been stitched"),
    ("丸盆用の飾り脚", "内側に折り畳めるらしい"),
    ("lattice roller sleeve", "may have reached the sample shelf"),
    ("注ぎ口用の紙栓", "湿し箱にあるらしい"),
    ("chime press handle", "might now fit the square socket"),
    ("柚子板用の額縁", "塗料が乾いたらしい"),
    ("crank lantern case", "may have a brass latch"),
    ("花筒の飾り輪", "交換されたらしい"),
    ("mica ticket display mount", "may tolerate damp air"),
    ("梅輪用の輸送台", "引き渡したらしい"),
    ("counter shutter sample slat", "may have been sanded"),
]
PREVIOUS_LOTS = ["浅籠用と深籠用の敷き紙を裁断", "lettered both rows",
                 "両面とも校正", "weighed both bins", "両棚の布を裾上げ", "punched both metals"]
ROOMS = ["杏蔵", "細波室", "橙棚間", "繭庫", "綿毛間", "灰峰庫", "苔色室",
         "oat gallery", "almond loft", "dune nook", "heather court", "willow bay", "rye annexe"]
ITEMS = [("紡ぎ駒","spindle whorl"),("格子針","weaving pin"),("貝の留め具","shell toggle"),
         ("木の指輪","wooden thimble"),("陶のへら","ceramic scraper"),("青銅の型紙","bronze template"),
         ("細紐巻","string bobbin"),("刷り玉","printing dabber"),("銀紙片","foil sample"),
         ("縫糸皿","thread dish"),("帆布帯","canvas strap"),("骨筆","bone stylus"),
         ("柳板","willow board"),("塩釉片","salt-glaze chip"),("縞紙","striped paper")]
COLORS = [("菜の花色","butter yellow"),("鳶色","russet"),("浅葱色","pale teal"),
          ("蘇芳色","sappan red"),("月白","moon white"),("柳色","willow green")]


def hash_key(value):
    return hashlib.sha256((SEED + ":" + value).encode()).hexdigest()


def choose(key, values):
    return values[int(hash_key(key), 16) % len(values)]


def originals():
    return json.loads((ROOT/"memory.json").read_text(encoding="utf-8"))


def hot_records(rows):
    return [r for lang in ("ja","en") for r in
            sorted((r for r in rows if r["kind"]=="rule" and r["language"]==lang),
                   key=lambda r: hash_key("hot:"+r["id"]))[:2]]


def generate(rows):
    base = {r["id"]:r for r in rows}
    for i, sibling in enumerate(SIBLINGS):
        record = base[f"n{(1+i if i < 6 else 19+i):02}"]
        for edition in range(1,13):
            for family, target in (("sample",record["target"]),("sibling",sibling)):
                key = f"{family}-{i}-{edition}"
                ja = record["language"] == "ja"
                shade = choose("ribbon:"+key,COLORS)[0 if ja else 1]
                scene = f"寸法教室の模型演習・台{edition}" if ja else f"model-fitting class, station {edition}"
                text = (f"{scene}で{target}の縮尺模型を扱うときだけ、{shade}の帯を模型台に留める。工房の実作業は対象外。"
                        if ja else f"In {scene} only, fasten a {shade} strip to the base of the scale model of {target}. Excludes ordinary workshop work.")
                yield key, family, dict(kind="rule",scene=scene,target=target,action=record["action"],
                    text=text,condition="scale-model-class",exceptions=["ordinary workshop work"])

    for i, alternatives in enumerate(FORMER):
        ref = f"n{7+i:02}"
        target = base[ref]["target"]
        # Keep the same six historical values but vary their chronological order by seed.
        for n, value in enumerate(sorted(alternatives,key=lambda v:hash_key(ref+v))):
            at = f"2042-{n+1:02}-14T08:00:00Z"
            text = (f"{at[:7]} 当時の{target}の選定値は「{value}」。この版は改訂済み。"
                    if i%2==0 else f"The {at[:7]} decision for {target} was {value}. This edition was superseded.")
            yield f"former-{i}-{n}", "former", dict(kind="decision",target=target,text=text,
                at=at,state="superseded",speech="directive",successor=ref if n==5 else f"former-{i}-{n+1}")

    for i,(item,claim) in enumerate(PROTOTYPES):
        for n in range(1,19):
            target = f"{item}・試作品{n}" if i%2==0 else f"{item}, prototype {n}"
            text = (f"当番の話では{target}は{claim}。試作品についての伝聞のみ。"
                    if i%2==0 else f"The shift heard the {target} {claim}. An unverified prototype report.")
            yield f"prototype-{i}-{n}", "report", dict(kind="fact",target=target,text=text,
                                                      certainty="reported",source_role="sample attendant")
    for i,result in enumerate(PREVIOUS_LOTS):
        target = base[f"n{19+i:02}"]["target"]
        for n in range(20):
            day = (date(2042,1,3) + timedelta(days=16*n)).isoformat()
            text = (f"{day}の旧ロットに限り、{target}は{result}済み。他期には繰り越さない。"
                    if i%2==0 else f"For {target} we {result} on the closed lot dated {day} only.")
            yield f"lot-{i}-{n}", "closed-lot", dict(kind="experience",target=target,text=text,
                                                    at=day+"T13:00:00Z",period=day)
    for ri,room in enumerate(ROOMS):
        ja = ri < 7
        for j,item in enumerate(ITEMS):
            for n in range(1,4):
                key = f"stock-{ri}-{j}-{n}"
                shade = choose(key,COLORS)[0 if ja else 1]
                target = f"{room}の{item[0]}第{n}組" if ja else f"{item[1]} set {n} in the {room}"
                at = f"2043-05-{1+(ri+j+n)%7:02}T10:00:00Z"
                for k,text,fields in [
                    ("rule",f"{target}を戻す前に{shade}の紐で台紙に結ぶ。" if ja else
                     f"Tie the {target} to its card with {shade} twine before returning it.",
                     dict(scene="after display",action="return",exceptions=[])),
                    ("decision",f"{target}の見出し枠は{shade}で確定した。" if ja else
                     f"The caption border for the {target} was settled as {shade}.",dict(at=at,state="decided",speech="directive")),
                    ("fact",f"{target}の台に脚を足したそうだ。" if ja else
                     f"The attendant heard that feet were added to the stand for the {target}.",dict(certainty="reported")),
                    ("experience",f"{target}の幅に合わせ、見本紙に折り筋を二本付けた。" if ja else
                     f"We scored two fold lines on the sample paper to fit the {target}.",dict(at=at)),
                ]:
                    yield key+"-"+k, "bulk", dict(kind=k,target=target,text=text,**fields)


def handle(key):
    return "w-"+hash_key("id:"+key)[:16]


def build():
    rows = originals()
    catalog = [(r["id"],"original",r) for r in rows] + list(generate(rows))
    result = []
    for key,family,record in sorted(catalog,key=lambda e:hash_key("shelf:"+e[0])):
        r = dict(record,id=handle(key))
        # The language tag is authoring metadata; no origin-specific marker in the ledger.
        r.pop("language",None)
        for field in ("evidence","successor"):
            if field in r:
                r[field] = handle(r[field])
        result.append((key,family,r))
    return result


def archive_bytes(rows):
    return b"".join(json.dumps(r,sort_keys=True,ensure_ascii=False,separators=(",",":")).encode()+b"\n"
                    for _,_,r in rows)


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description="Generate the second frozen ledger")
    parser.add_argument("output",type=Path)
    parser.parse_args().output.write_bytes(archive_bytes(build()))
