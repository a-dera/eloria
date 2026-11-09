"""Writes public/demo/exemple.pdf: a two-page PDF with selectable text,
to test PDF annotation in mock mode (npm run dev). No dependency."""

import os

PAGES = [
    [
        "Graph Neural Networks for Mobile Money Fraud Detection",
        "",
        "Fraud rings tend to form dense subgraphs with high transaction velocity.",
        "Our dataset covers 2.3M transactions from three operators.",
        "We compare GNN models with gradient boosting baselines.",
    ],
    [
        "Results",
        "",
        "The GNN detects organised fraud with a large gain in recall.",
        "Isolated fraud remains hard to detect without behavioural features.",
    ],
]


def esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def build() -> bytes:
    objs: list[bytes] = []
    n_pages = len(PAGES)
    # 1 catalogue, 2 pages, 3 font, then (page, content) per page
    kids = " ".join(f"{4 + 2 * i} 0 R" for i in range(n_pages))
    objs.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objs.append(f"<< /Type /Pages /Kids [{kids}] /Count {n_pages} >>".encode())
    objs.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    for i, lines in enumerate(PAGES):
        content = ["BT", "/F1 13 Tf", "72 760 Td", "18 TL"]
        for j, line in enumerate(lines):
            content.append(f"({esc(line)}) Tj" if j == 0 else f"T* ({esc(line)}) Tj")
        content.append("ET")
        stream = "\n".join(content).encode("latin-1")
        objs.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {5 + 2 * i} 0 R >>".encode())
        objs.append(b"<< /Length " + str(len(stream)).encode() + b" >>\nstream\n" + stream + b"\nendstream")
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for k, o in enumerate(objs, start=1):
        offsets.append(len(out))
        out += f"{k} 0 obj\n".encode() + o + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


if __name__ == "__main__":
    root = os.path.join(os.path.dirname(__file__), "..", "public", "demo")
    os.makedirs(root, exist_ok=True)
    with open(os.path.join(root, "exemple.pdf"), "wb") as f:
        f.write(build())
    print("public/demo/exemple.pdf")
