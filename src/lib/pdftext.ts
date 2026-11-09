// Text of a PDF, extracted by PDF.js (like the reader): kept by the core for
// search, Ask Eloria and reading the abstract when no online database provides it.

import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

/** Text of the pages (at most `maxPages`), lines and spaces rebuilt from positions. */
export async function docText(doc: pdfjs.PDFDocumentProxy, maxPages = 80): Promise<string> {
  const pages: string[] = [];
  for (let i = 1; i <= Math.min(doc.numPages, maxPages); i++) {
    const content = await (await doc.getPage(i)).getTextContent();
    let s = "";
    let lastX: number | null = null;
    let lastY = 0;
    for (const it of content.items) {
      if (!("str" in it)) continue;
      const x = it.transform[4];
      const y = it.transform[5];
      if (lastX !== null && s && !/\s$/.test(s)) {
        if (Math.abs(y - lastY) > 2) s += "\n";
        else if (x - lastX > 1) s += " ";
      }
      s += it.str;
      lastX = x + it.width;
      lastY = y;
      if (it.hasEOL) s += "\n";
    }
    pages.push(s);
  }
  return pages.join("\n\n");
}

/** Text of a PDF reachable by URL. */
export async function urlText(url: string): Promise<string> {
  const data = await fetch(url).then((r) => {
    if (!r.ok) throw new Error(String(r.status));
    return r.arrayBuffer();
  });
  const task = pdfjs.getDocument({ data });
  try {
    return await docText(await task.promise);
  } finally {
    void task.destroy();
  }
}
