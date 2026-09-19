# Archived BLS price-index sources

Archived primary text for the index series the `projection` blocks of `params/vintages/`
read (`DOMAIN-MODEL.md` §15, `TESTING.md` §3.1 and §12 item 1, `docs/contributing.md` §1.1).
Everything here is a byte-exact copy of a file published by the Bureau of Labor Statistics,
retrieved over HTTPS from a publisher-controlled host (`download.bls.gov`, `www.bls.gov`).
No third-party copy is included.

**Licence basis:** every file below is a work of the United States Government (Bureau of
Labor Statistics, U.S. Department of Labor) and is not subject to copyright protection in
the United States (17 USC 105). No licence is asserted over the bytes and they are outside
the project's Apache-2.0 grant (see the root `NOTICE`); the archive records them verbatim.
This is the same basis `params/provenance/INDEX.toml` states as `licence_basis` for the
IRS, US Code and Congress documents.

**Nothing here is verified.** Every file was fetched and checksummed by an AI-assisted
session; the statutory reading that selects these series is recorded in the working log and
is on the `TESTING.md` §5.2 hand-verification list, gated at **M0, before the first
`federal-2026` vintage locks**. No vintage may cite a file below until a human has read the
statute and confirmed the series identity (ADR-022, `docs/contributing.md` §3.2).

All files retrieved **2026-09-18**. Sizes are bytes; digests are SHA-256 of the archived copy.

## Index series (BLS flat-file time-series database)

| File | Series / contents | URL | Bytes | SHA-256 |
|---|---|---|---|---|
| `time-series/su.data.1.AllItems` | Chained CPI for All Urban Consumers (C-CPI-U), all published item series incl. **`SUUR0000SA0`** — All items, U.S. city average, not seasonally adjusted, December 1999 = 100; monthly 1999-M12 … 2026-M08 | https://download.bls.gov/pub/time.series/su/su.data.1.AllItems | 405440 | `c40304234a1e838cffe4ba88567a997b91595c223613f2584aa0cab3191f3f75` |
| `time-series/su.series` | `su` series catalogue: series id, seasonality, periodicity, base period, title, begin/end period | https://download.bls.gov/pub/time.series/su/su.series | 4914 | `7da3ac42aef524b2cc21fe2cb8f6ab71124ae808db699f2b4d5919b306bafbcc` |
| `time-series/su.footnote` | `su` footnote-code table — **this is the vintage marker**: `I` = Initial, `U` = Interim, empty = final | https://download.bls.gov/pub/time.series/su/su.footnote | 51 | `732d760a9e80af7637f7bcded8f4828dd0422b46faa98e03099e8fd3d792170f` |
| `time-series/su.txt` | `su` survey definition and file layout | https://download.bls.gov/pub/time.series/su/su.txt | 10689 | `18502bd809bcec692204a86353b7b4b9612ae82a35a8067580e5c1671fae788b` |
| `time-series/cu.data.1.AllItems` | CPI (CU), all-items series incl. **`CUUR0000SA0`** — CPI-U, All items, U.S. city average, not seasonally adjusted, 1982-84 = 100; monthly 1913-M01 … 2026-M08 | https://download.bls.gov/pub/time.series/cu/cu.data.1.AllItems | 2688415 | `47507ab13d9366d6e9f6a85a50bbd863179c97fa51c92ecb737875455d6ddc0f` |
| `time-series/cu.txt` | `cu` survey definition and file layout | https://download.bls.gov/pub/time.series/cu/cu.txt | 14644 | `1394b7f4210fda3d6dab62437bae5bd3664ed040612dd3df6268bc4e2911a471` |

`CUUR0000SA0` is archived because 26 USC 1(f)(3)(A)(ii), (f)(3)(B), (f)(4) and (f)(5) name
the CPI-U by reference. It is **statutorily named and numerically inert**: for a base year of
2016 the CPI-U terms cancel exactly, and for a base year after 2016 subparagraph (f)(3)(C)
removes them from the clause. See the working log for the arithmetic.

## Publication, vintage and revision documentation

| File | What it is | URL | Bytes | SHA-256 |
|---|---|---|---|---|
| `cpi_09112025.htm` | CPI news release **USDL-25-1356**, *Consumer Price Index — August 2025*, embargoed until 8:30 a.m. ET Thursday, **September 11, 2025**. Carries the initial August 2025 C-CPI-U, which is the publication event 26 USC 1(f)(6)(A) uses to freeze the inputs to the calendar-year 2026 adjustment; its Technical Note and Table 5 footnote state the C-CPI-U revision schedule | https://www.bls.gov/news.release/archives/cpi_09112025.htm | 1362710 | `f376b178ee4e16a7cc2ae6810e2164cad96aa8da156900623b2ff780a78f26bf` |
| `chained-cpi-questions-and-answers.htm` | *Frequently Asked Questions about the Chained Consumer Price Index for All Urban Consumers (C-CPI-U)* — published series list (table 1), construction, revision practice | https://www.bls.gov/cpi/additional-resources/chained-cpi-questions-and-answers.htm | 71607 | `7dddb81e1b25b28d9eb638a13d35117ba15a658bdfd980ee1702e43d124bde2e` |
| `2025-federal-government-shutdown-impact-cpi-faq.htm` | *2025 federal government shutdown impact on the Consumer Price Index* — what was and was not collected, and how the missing October 2025 observation is represented in BLS products | https://www.bls.gov/cpi/additional-resources/2025-federal-government-shutdown-impact-cpi-faq.htm | 76572 | `fada646ba27ac327b1719bd2f9c442b2fd1d3d9ae2d062ba8a4f6464398584ca` |
| `2025-federal-government-shutdown-impact-cpi.htm` | *2025 Federal Government Shutdown Impact on Consumer Expenditure Surveys (CE) and Consumer Price Index (CPI)* — states that the missing CE data affect the **final revisions of the 2025 Chained CPI-U indexes** and records the weight-adjustment approach BLS selected | https://www.bls.gov/cpi/additional-resources/2025-federal-government-shutdown-impact-cpi.htm | 70110 | `d1ec58a17d57e70a17cb363a61bc4ec0fca5a646a682361637f90c21c46d8734` |
| `2025-lapse-revised-release-dates.htm` | *Revised news release dates following the 2025 and 2026 lapses in appropriations* — records that the October 2025 Consumer Price Index release was **Canceled** | https://www.bls.gov/bls/2025-lapse-revised-release-dates.htm | 61476 | `66a58a4ded748779939df9d7e2c92b0d8c308492b8355621375a4a996ab985c9` |

## Retrieval notes

- `download.bls.gov` and `www.bls.gov` return **HTTP 403** to a plain `curl` request and to a
  request carrying only a browser `User-Agent`. They serve normally once the request also
  carries `Accept`, `Accept-Language`, `Sec-Fetch-Dest: document`, `Sec-Fetch-Mode: navigate`,
  `Sec-Fetch-Site: none`, `Upgrade-Insecure-Requests: 1` and negotiates compression. `HEAD` is
  refused on both hosts even then; only `GET` succeeds. Every URL above was retrieved by `GET`.
- The BLS public data API (`https://api.bls.gov/publicAPI/v1/timeseries/data/`) serves the same
  series without a key and was used as an independent cross-check. Every monthly observation of
  `SUUR0000SA0` and `CUUR0000SA0` for 2015–2026 agreed with the flat files above, value for
  value. API responses are **not** archived here: their JSON carries a per-request
  `responseTime` field, so they are not byte-reproducible.
- The archived web pages are BLS templates and carry the site's navigation chrome around the
  substantive text. They are archived whole and unedited.
- The statute itself (26 USC 1) is **not** archived here. It is a tax document and is archived
  by the tax-document task as `params/provenance/usc/usc26-s1.html`, catalogued in
  `params/provenance/INDEX.toml` under key `USC1`. Note that the OLRC `view.xhtml` URL embeds
  per-session JSF state in its own markup and so is **not byte-reproducible**: two fetches of
  it on 2026-09-18 differ in 36 lines, all of them `jsessionid` or `javax.faces.ViewState`,
  while the tag-stripped text is byte-identical. The working log records the measurement.
- **This file and `params/provenance/INDEX.toml` are two catalogues of one archive.** `INDEX.toml`
  covers the IRS, USC and Congress documents and deliberately does not list the files here;
  this file covers `bls/` and does not list those. They should be merged, or one made
  normative, before any vintage locks.
