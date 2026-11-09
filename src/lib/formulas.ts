// Built-in formula library (metrics, probability, learning, graphs,
// explainability) and symbol palette of the equation editor.

export interface BuiltinFormula {
  group: string;
  name: string;
  latex: string;
}

export const BUILTIN: BuiltinFormula[] = [
  { group: "Métriques", name: "Précision", latex: "P = \\frac{TP}{TP + FP}" },
  { group: "Métriques", name: "Rappel", latex: "R = \\frac{TP}{TP + FN}" },
  { group: "Métriques", name: "Score F1", latex: "F_1 = \\frac{2 \\, P \\, R}{P + R}" },
  { group: "Métriques", name: "Score F-bêta", latex: "F_\\beta = (1 + \\beta^2) \\, \\frac{P \\, R}{\\beta^2 P + R}" },
  { group: "Métriques", name: "Exactitude", latex: "\\mathrm{Acc} = \\frac{TP + TN}{TP + TN + FP + FN}" },
  { group: "Métriques", name: "Spécificité", latex: "\\mathrm{Spec} = \\frac{TN}{TN + FP}" },
  { group: "Métriques", name: "Coefficient de Matthews", latex: "\\mathrm{MCC} = \\frac{TP \\cdot TN - FP \\cdot FN}{\\sqrt{(TP+FP)(TP+FN)(TN+FP)(TN+FN)}}" },
  { group: "Métriques", name: "Aire sous la courbe ROC", latex: "\\mathrm{AUC} = \\int_0^1 \\mathrm{TPR}\\big(\\mathrm{FPR}^{-1}(u)\\big)\\, du" },
  { group: "Probabilités", name: "Théorème de Bayes", latex: "P(A \\mid B) = \\frac{P(B \\mid A)\\, P(A)}{P(B)}" },
  { group: "Probabilités", name: "Espérance", latex: "\\mathbb{E}[X] = \\sum_{x} x \\, P(X = x)" },
  { group: "Probabilités", name: "Variance", latex: "\\operatorname{Var}(X) = \\mathbb{E}\\big[(X - \\mathbb{E}[X])^2\\big]" },
  { group: "Probabilités", name: "Score z", latex: "z = \\frac{x - \\mu}{\\sigma}" },
  { group: "Probabilités", name: "Entropie", latex: "H(X) = -\\sum_{x} p(x) \\log p(x)" },
  { group: "Probabilités", name: "Divergence de Kullback-Leibler", latex: "D_{\\mathrm{KL}}(P \\,\\|\\, Q) = \\sum_{x} P(x) \\log \\frac{P(x)}{Q(x)}" },
  { group: "Apprentissage", name: "Sigmoïde", latex: "\\sigma(z) = \\frac{1}{1 + e^{-z}}" },
  { group: "Apprentissage", name: "Régression logistique", latex: "P(y = 1 \\mid \\mathbf{x}) = \\sigma(\\mathbf{w}^\\top \\mathbf{x} + b)" },
  { group: "Apprentissage", name: "Softmax", latex: "\\operatorname{softmax}(\\mathbf{z})_i = \\frac{e^{z_i}}{\\sum_{j=1}^{K} e^{z_j}}" },
  { group: "Apprentissage", name: "Entropie croisée binaire", latex: "\\mathcal{L} = -\\frac{1}{N} \\sum_{i=1}^{N} \\big[ y_i \\log \\hat{y}_i + (1 - y_i) \\log (1 - \\hat{y}_i) \\big]" },
  { group: "Apprentissage", name: "Perte focale (classes déséquilibrées)", latex: "\\mathcal{L}_{\\text{focal}} = -\\alpha_t (1 - p_t)^{\\gamma} \\log p_t" },
  { group: "Apprentissage", name: "Descente de gradient", latex: "\\theta_{t+1} = \\theta_t - \\eta \\, \\nabla_\\theta \\mathcal{L}(\\theta_t)" },
  { group: "Apprentissage", name: "Régularisation L2", latex: "\\mathcal{L}_{\\text{reg}} = \\mathcal{L} + \\lambda \\lVert \\mathbf{w} \\rVert_2^2" },
  { group: "Graphes", name: "Passage de messages (GNN)", latex: "\\mathbf{h}_v^{(k)} = \\phi\\Big(\\mathbf{h}_v^{(k-1)}, \\bigoplus_{u \\in \\mathcal{N}(v)} \\psi\\big(\\mathbf{h}_u^{(k-1)}, \\mathbf{e}_{uv}\\big)\\Big)" },
  { group: "Graphes", name: "Propagation GCN", latex: "H^{(l+1)} = \\sigma\\big(\\tilde{D}^{-\\frac{1}{2}} \\tilde{A} \\tilde{D}^{-\\frac{1}{2}} H^{(l)} W^{(l)}\\big)" },
  { group: "Graphes", name: "Attention (GAT)", latex: "\\alpha_{uv} = \\frac{\\exp\\big(\\mathrm{LeakyReLU}(\\mathbf{a}^\\top [W\\mathbf{h}_u \\,\\|\\, W\\mathbf{h}_v])\\big)}{\\sum_{k \\in \\mathcal{N}(u)} \\exp\\big(\\mathrm{LeakyReLU}(\\mathbf{a}^\\top [W\\mathbf{h}_u \\,\\|\\, W\\mathbf{h}_k])\\big)}" },
  { group: "Graphes", name: "PageRank", latex: "PR(v) = \\frac{1 - d}{N} + d \\sum_{u \\in \\mathcal{B}(v)} \\frac{PR(u)}{L(u)}" },
  { group: "Explicabilité", name: "Valeur de Shapley", latex: "\\phi_i = \\sum_{S \\subseteq F \\setminus \\{i\\}} \\frac{|S|!\\,(|F| - |S| - 1)!}{|F|!} \\big[ f(S \\cup \\{i\\}) - f(S) \\big]" },
  { group: "Explicabilité", name: "Attribution additive (SHAP)", latex: "g(z') = \\phi_0 + \\sum_{i=1}^{M} \\phi_i z'_i" },
];

/** Symbol palette: [display, inserted code]. `▢` marks where to put the cursor. */
export const SYMBOLS: { group: string; items: [string, string][] }[] = [
  {
    group: "Grec",
    items: (
      [
        ["α", "alpha"], ["β", "beta"], ["γ", "gamma"], ["δ", "delta"], ["ε", "epsilon"], ["θ", "theta"], ["λ", "lambda"], ["μ", "mu"],
        ["σ", "sigma"], ["τ", "tau"], ["φ", "phi"], ["ω", "omega"], ["Σ", "Sigma"], ["Π", "Pi"], ["Δ", "Delta"], ["Ω", "Omega"],
      ] as [string, string][]
    ).map(([glyph, name]) => [glyph, `\\${name}`]),
  },
  {
    group: "Structures",
    items: [
      ["a/b", "\\frac{▢}{}"],
      ["√", "\\sqrt{▢}"],
      ["xⁿ", "^{▢}"],
      ["xₙ", "_{▢}"],
      ["Σ", "\\sum_{i=1}^{n} ▢"],
      ["Π", "\\prod_{i=1}^{n} ▢"],
      ["∫", "\\int_{a}^{b} ▢ \\, dx"],
      ["x̂", "\\hat{▢}"],
      ["x̄", "\\bar{▢}"],
      ["𝐱", "\\mathbf{▢}"],
      ["𝔼", "\\mathbb{E}[▢]"],
      ["cas", "\\begin{cases} ▢ & \\text{si } \\\\  & \\text{sinon} \\end{cases}"],
      ["matrice", "\\begin{pmatrix} ▢ & \\\\ & \\end{pmatrix}"],
      ["argmax", "\\operatorname*{arg\\,max}_{▢}"],
    ],
  },
  {
    group: "Relations",
    items: [
      ["≤", "\\leq"],
      ["≥", "\\geq"],
      ["≠", "\\neq"],
      ["≈", "\\approx"],
      ["∈", "\\in"],
      ["⊂", "\\subset"],
      ["→", "\\to"],
      ["⇒", "\\Rightarrow"],
      ["∀", "\\forall"],
      ["∃", "\\exists"],
      ["∞", "\\infty"],
      ["∂", "\\partial"],
      ["∇", "\\nabla"],
      ["·", "\\cdot"],
      ["×", "\\times"],
    ],
  },
];

/** Cell array -> LaTeX (booktabs) or Markdown. */
export function tableCode(cells: string[][], opts: { header: boolean; align: ("l" | "c" | "r")[]; latex: boolean; caption: string; label: string }): string {
  const rows = cells.map((r) => r.map((c) => c.trim()));
  if (opts.latex) {
    const esc = (s: string) => s.replace(/([&%#_])/g, "\\$1");
    const body = rows.map((r) => `    ${r.map(esc).join(" & ")} \\\\`);
    if (opts.header && body.length) body.splice(1, 0, "    \\midrule");
    return [
      "\\begin{table}[ht]",
      "  \\centering",
      `  \\begin{tabular}{${opts.align.join("")}}`,
      "    \\toprule",
      ...body,
      "    \\bottomrule",
      "  \\end{tabular}",
      ...(opts.caption.trim() ? [`  \\caption{${opts.caption.trim()}}`] : []),
      ...(opts.label.trim() ? [`  \\label{tab:${opts.label.trim().replace(/^tab:/, "")}}`] : []),
      "\\end{table}",
    ].join("\n");
  }
  const esc = (s: string) => s.replace(/\|/g, "\\|") || " ";
  const line = (r: string[]) => `| ${r.map(esc).join(" | ")} |`;
  const sep = `| ${opts.align.map((a) => (a === "c" ? ":---:" : a === "r" ? "---:" : "---")).join(" | ")} |`;
  const [first, ...rest] = rows;
  const head = opts.header ? first : opts.align.map(() => "");
  const tail = opts.header ? rest : rows;
  return [line(head), sep, ...tail.map(line)].join("\n");
}
