# Typography for the web app

Rules for text in the web app, taken from Matthew Butterick's *Practical Typography* ([summary of key rules](https://practicaltypography.com/summary-of-key-rules.html)), with the Tailwind values this app uses for each. The frontend review checks changes against them (FE-R9). Written 2026-10-26, after the question page restyle.

Most of the app is reading: question wording, answers, rubrics and Claude's feedback. These rules matter most there.

## Rules

| Rule | Practical Typography | In this app |
|---|---|---|
| **Body size** | 15–25 px on screens | `text-base` (16 px) for reading text such as answers and feedback. `text-sm` (14 px) only for secondary text: metadata, table cells, side panels. |
| **Line spacing** | 120–145% of the font size | `leading-relaxed` (1.625) for paragraphs. Butterick's range is a guide; slightly looser reads well at 16 px on screen. Headings are tighter (`leading-tight`). |
| **Line length** | 45–90 characters per line, spaces included | Keep reading columns to about `max-w-[42rem]` (roughly 70–80 characters at 16 px). Never let paragraphs run the full page width. |
| **Emphasis** | Bold or italic, used sparingly, never both together; no underlining except links | One emphasis per phrase. Headings use weight, not weight plus italic. |
| **Paragraph spacing** | Either space between paragraphs or first-line indents, never both | Space between paragraphs (`space-y-*`); no indents. |
| **All caps** | Only for less than a line of text, with 5–12% extra letter spacing | Prefer sentence-case headings. If a short label must be caps, use at least `tracking-wider` (0.05em); `tracking-wide` (0.025em) is too tight. |
| **Headings** | (Butterick: few levels, distinguished by size and weight, with more space above than below) | Sentence case. `text-3xl font-bold tracking-tight` for page titles, `text-lg font-semibold` for sections, `font-semibold` for sub-sections. |

## Layout conventions alongside the rules

Taken from [Stripe's documentation](https://docs.stripe.com/payments/accept-a-payment), a well-regarded reading layout:
- A large bold title, with small context (level, status) above it rather than beside it.
- Sentence-case section headings instead of small all-caps labels.
- A narrow reading column, with secondary information in a side panel.
- Asides and citations as a block with a coloured left border.

## Known gaps

- `web/src/components/home/TodayTiles.tsx` uses `uppercase tracking-wide` for tile labels, which is below the recommended letter spacing.
