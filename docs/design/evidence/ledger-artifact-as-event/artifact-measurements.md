# Artifact intake measurements

P — retained prior measurements as-of 2026-09-08 04:05 UTC, identified by artifact-inventory.json hashes; labels below describe that prior lane, not a fresh live census or replay of artifact claims. Round-1 header re-pricing is recorded separately in ledger-artifact-as-event.md §10 and artifact-fix-verification.json.

## Review inventory

| Population | Files | Bytes | Min | Median | Max | Front matter | RESULT-first | Over 4096 B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| wave-1 immediate reviews/*.md | 54 | 586909 | 1376 | 7643.5 | 66097 | 0 | 0 | 50 |
| wave-2 immediate reviews/*.md | 38 | 381736 | 3441 | 8487.5 | 39244 | 0 | 0 | 36 |

S: all 105 captured files had unchanged inode/size/mtime around their individual reads. No cross-file atomic snapshot is claimed.
S: recursive regular-file inventory (excluding symlinks) counted wave-1 reviews 1366 and wave-2 reviews 72; these include images/logs, not additional qualifying prose events.

## Completions

| Task | UTF-8 summary bytes | Existing carrier contains (I: manual coding) |
|---|---:|---|
| 3 | 290 | Accepted measurement; source tip; four tool observations and probe basis |
| 4 | 138 | Accepted inventory; full commit; 169/180 lines and 65 pointers |
| 5 | 247 | Accepted document; full commit; 304/350 lines and 29 assertions |
| 6 | 572 | T6a output; commit; 57 checks and 205/260 lines; explicit unresolved privacy seam |
| 8 | 307 | Accepted readiness; full commit; timings/77 of 80 lines; three explicit unvalidated areas |
| 7 | 131 | Two review identities and bounded-correction disposition |
| 9 | 148 | Review result/rejection; candidate and report tips; 10 findings/120 renders; gate unavailable |

S: 7 completion summaries, 1833 UTF-8 bytes total, median 247, min 131, max 572, all <=4096. Journal: 321 events.
S: 106 archived inbox entries; 0 raw text bodies begin RESULT. Six nested idle_notification summaries start [to ...] RESULT; their nested result fields are turn-end prose, not proven copies of sent messages.
S: selected six turn-end body byte counts: 3107, 2506, 1599, 2035, 1877, 2582.

## Byte-estimator inputs

P: original future NOTES header was 464 UTF-8 bytes / 20 lines before Round-1 (preserved in ledger-artifact-as-event.before-round1.md); current future-notes-header.txt is re-priced in the report; it is not an accepted event or an implementation.
S: selected wave-2 ledger row #9 is 1297 bytes (ledger.md:122), #49 is 614 bytes (:161), #51 is 263 bytes (:162), excluding line terminators. Rows are whole composite rows, not per-fact edits.
S: 72 numeric-prefixed pipe lines total 43938 bytes; widths are 61×10, 4×11, 2×12, 1×9, 1×3, 3×5. This syntactic population includes non-board rows and malformed rows; never use it as a count of tasks or facts.
I: estimate for a matched fact i: old=M_i+L_i; new=M_i+F_i+N_i+R_i; net=L_i-F_i-N_i-R_i. M=existing message prose; L=authored ledger edit span; F=added metadata; N=new prose absent from existing artifact; R=revision/retry repair authoring. Existing artifact bytes cancel. Unknown M/L are not zero.
U — UNVERIFIED: final-wave exact messages and ledger edit spans were not available in the early archive. No measured savings rate or whole-wave per-fact mean can be computed from these inputs.

## Identity manifest

| Path | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| /home/mstie/projects/taurjob/docs/wave-1/reviews/R15-altitude.md | 4328 | 60 | `e070dbc43afc406187aa047b2affc529ae5b01f0feafc421bb62ef0eb7d592b0` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T1-altitude-reviewer.md | 8398 | 124 | `7600312e486c02bff5ca7ab464abd6830d1b0fdf1b8a392203c589677af25286` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T1-altitude.md | 5667 | 94 | `c19c327d10a17fe0256ab56c858f722d9438bee4ed8e105c5375e0a063f8d301` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T11-structural.md | 7345 | 117 | `ca67a21a19f0a1e650df0ad4dee3fccad86bdfa19097f441a3a3a46b9e6c2618` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T15-A-structural.md | 7304 | 100 | `5567893ce400f26dfa43fac1188012879e4d4da2a421a78ffc74e693a109035d` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T15-B-structural.md | 6318 | 80 | `0abbb45d04432c825c4cf2c9050baad701dc24c3d24e82def262369b8508b9a9` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T15-acceptance.md | 7496 | 86 | `b621a9a3ab58c5cb9c9a1620b166316d6e35314a6484ff0659d805fee6babcb5` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T16-A-structural.md | 7131 | 100 | `846ab7c44ff0bc7bd0989929d4f04294a757d02cda50e573357d29ab7a45a628` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T16-B-structural.md | 6501 | 93 | `3f71fc62b9a91f4609cf8d3ddb3e60b7dfaef37a514915d57070880ffad02470` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T16-acceptance.md | 6028 | 73 | `d8d8762b9dc87aac0e7639509fdc11937bb1f95d2f8a7a8ceb92a02001807e18` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T19-A-structural.md | 7614 | 100 | `ac48b8996260fa049411d16c9eef9961cd2c813ba8eb9b32e9c23dd6ea17f33c` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T19-A2-structural.md | 5348 | 58 | `5f5e2b0d91a6fa78657985bcf245eeee264b3ae7d028c62a648f5947844da96b` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T19-A3-structural.md | 3512 | 28 | `90cc620397a18f4a853f58de99a2df8d3232b60a5814435a6756f129c19e999d` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T19-B-structural.md | 10587 | 127 | `90395c5b547ddcdac74abcce97e5e9ab52cd8b750e7caba21a66af6f601658ce` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T19-acceptance.md | 15160 | 224 | `c0015b7d3b518e0c78aaaefa8c2ff422687173ad582b358c9d4cbcb9e7a2a3ab` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T1r2-altitude-claude-draft.md | 12012 | 151 | `7de5e2fd389156f0bbea247c504b5936c24d4f3f753048a1152214f04d99721e` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T1r2-altitude-claude.md | 8877 | 122 | `9f23e51e135c7e70f3d4ca9c7284f1cf0f39104a13b740125eb82f726805e059` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T2-gpt-review.md | 11769 | 75 | `b805446fe60eae5929ca269d2413b2e714a59eb3613cba49dd900e0289a4d1b9` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T2-lead-walkthrough.md | 1376 | 25 | `9819121fd5715f244529118177bf645ac1a2d9fe930298290dfa32ca6fa18562` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T2-product-review.md | 9389 | 142 | `9b4bc511415b325c5c31fd54715b460c08785fb285845bc2bda41cf7af7ccc11` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-live.md | 15880 | 113 | `86325d7d55bae418cc70636e1885e433488c2348e70748a5081589625a67bae5` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-rechecks-gpt-review.md | 6493 | 28 | `b5df0bbd1504e732f2ad503a9975236fd881c411ae6d3cbd30cc4c4aa1f74ca8` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-results-gpt-review.md | 11492 | 50 | `7972becea22fd3bf9659a362f217752850185f88b250e18e0b05545b9beb3acb` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-results.md | 14871 | 120 | `9fb2030ec1123673d77a45e252b3f4e1efa2d6e7dbdb3f7b1a72ea629dd24115` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-run-gpt-review.md | 11238 | 45 | `93638d9144e509ec52a393e6c912d11f07420668071f347eca7086ba9bc3250c` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-run.md | 14882 | 99 | `dde7da7fee9e02700cdb72497b7125ffdf631d92196f7935f54f2412ba26eeb1` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-setup-gpt-review.md | 10058 | 51 | `2398ca39a183cb7510d5a8ce044c9045bbc8b1c31886aa156cb475d8d0985dd3` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T20-setup.md | 12819 | 96 | `052494e5754a2130a2169b1f97b7d37a6c589e3511c61d5b893330c21683a370` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T21-A-acceptance.md | 7673 | 119 | `9998d63127bacd1db063805b829f2c372422d5d9602915bb847d5cc04c7e159e` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T21-A-structural.md | 7785 | 79 | `9c45b565d7b93dbe2b898d04ce67b977fa6bfde5c73e6286a4662bf92a4e73bb` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T21-B-structural.md | 10342 | 125 | `647d8f0cfc2cdd98602a118135175b51245dc172e9a85decc52dc87383852cbe` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T21-acceptance.md | 18135 | 271 | `0ac9909cff300bd0ff213735a2536b84df8c0a0109c9356c85dd02ceba0e27c7` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T22-acceptance.md | 6256 | 90 | `0666c0fd6814eb0acd4ee72ccd11a14acbe83940be11d097e252137fea0bbff1` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T22-structural.md | 5643 | 76 | `dfa0326e9704667d253b93b7b0eea5d273a511dbfbb8db5949eb9efea0f68a87` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T24-A-acceptance.md | 5997 | 99 | `e8fe81d07814a7949bbdc407464db18463bed1c87e402e84086e77f97df68419` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T24-A-structural.md | 6771 | 90 | `1980386f06fcd43963b767445de6c65d964f60bcd08f0431dc49e8e1bea44f12` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T24-B-structural.md | 7281 | 90 | `371034af3a27e3e4b3eb1f2bedf9d5732d4cccb21d537c13846376be5bf25ba2` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T24-acceptance.md | 5533 | 69 | `80a6900418907797ef415357895cd6dbf5a54387c991561242a7d480fadfec9c` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-cert-product-acceptance.md | 6078 | 56 | `ac976e7c2390d11e922945ddadc89ab0f341865e08ef53a41d6f91d4cdf6de8e` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-certification.md | 6765 | 53 | `cad6d579b01386713572ee907a0a32d6e7257a543b5315da877ba74e0d9167df` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-enrichment.md | 3466 | 47 | `66c106b82efa28683093924bb44b56f9755820cf5b957a6338a80db37693f29e` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-gpt-review.md | 8933 | 32 | `c6bfddf81d46f7a89941eef092837bd6a6fa0a7448ca0f3fa763178e3d4ec145` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-integration-evidence.md | 8775 | 71 | `9fed863b715a5136735425c817457395357f36b23f2a0f866ffd39403dc567a5` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-product-acceptance.md | 6195 | 79 | `bf03c84f58bbbe72eac711345713b77c8b067986eabc92714a8f52a1c2469c01` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-rerun-evidence.md | 10180 | 73 | `c61d7d5b929c42d662106c7538864256e9387f391dc4f0316f5d9b01d97603d4` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-rerun-gpt-review.md | 61140 | 386 | `8278513a43b95bde4eb15a38bd809fe5c1b18ecd2b0870b33117f1c176287ad6` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-rerun-product-acceptance.md | 7588 | 52 | `b5913b64a9c90df16e577e5713bdcb2bfa99270528229efd14fb16647ae23d22` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-rerun-structural.md | 27497 | 289 | `5feece0782465fb63ce0f935500b80360a76b989a97143d485a56c17d7a3cd0a` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T26-structural.md | 9161 | 106 | `22b6173b1dfc704be6d2616b093c0ca9266d2adaa727b1f1120d75d2d9e636fa` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T2a-gpt-review.md | 15349 | 82 | `2e8a909d3709f0a17ef6f561ba67da5e6710c835ecd9c16a202fb3b18763817e` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/T9-acceptance.md | 6108 | 88 | `dc4e9273db64c0b458953cb69606893fa404e645c12f7934fe127c1b3226b0cb` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/check-architecture-pass.md | 3638 | 26 | `42802cd33179b04e99edb0792cecb3e2a4c0ba7979ea7fe2f5c74ec7506d249a` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/integration-acceptance-plan-gpt-review.md | 8630 | 32 | `82674d41a53fc2a17480b1ee25f1b0408c11c2be47a7c2b4a20d3c9aa1fc7f59` |
| /home/mstie/projects/taurjob/docs/wave-1/reviews/integration-acceptance-plan.md | 66097 | 377 | `a52dc60e61f08324841fef79b422ae0568c6144d9f0179f9cb20038792ba13ee` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S1-triage.md | 8209 | 43 | `074dd2da6007767471fe639d53d4d0e269a3f35a3d5fe50b14034fce098588ef` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S2-findings-register.md | 39244 | 158 | `eb016cd3e2938584da4236e320e2a9932232284b8d0e7fe3c89e48df333441c3` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S2-sweep-altitude.md | 18008 | 250 | `9cef1997e1140f0acbd58827c71bd674aaea88d30f61ebd4d025711fe84d8825` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S2-sweep-judge.md | 15776 | 153 | `2749ca6cdc04ecbdcfdc817f197dac9b4f36c9a0f61b3514f204274fef68aaa6` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S3-S4-architecture-altitude.md | 8176 | 120 | `87e98e36340513b7bc49ed782c43099cea35ccddeacfd0ca67bcf26aa5b51f76` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S3-S4-design-review-judge.md | 15090 | 98 | `e53287cab1eaa7cf0a7e5d978e573248d05a7607542f6f85c4e9c757c02d8707` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S3-run-results-design-review.md | 4511 | 66 | `8ed11c7d6e87a6fc52fcd48ff212b749e40eee8b77e25cf4362ce473699774c9` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S3-setup-design-review.md | 4977 | 74 | `e1dc6bbcc995d5fc07285cbe9715a65b5ea16e2ca26716daaf04989004946e92` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/S4-notice-design-review.md | 4605 | 79 | `cd86d83c2c9a5877496f496c0afa1c78d73fe2e62c9b83a98297500cb6db741d` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T10-review-altitude.md | 8013 | 113 | `bef2271912373ce81106784c4a913501fc44602f274610a76eef0bd9f303ce9f` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T11-review-altitude.md | 7208 | 104 | `c4b3765c65df682ea199106169cff7cc614a47187766ff3e985e4803db4fa2cb` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T12-review-altitude.md | 8557 | 120 | `d609bf278622299395524486f4bb00c5166f921e6c34f05f0a939b8aba869f97` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T13-review-altitude.md | 3441 | 50 | `f66718a43f14de664bf0d76cac9ff37394ca79910e5b7180ff61d16e3453c2ae` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T14-review-judge.md | 8638 | 45 | `217bdeffc3677af859a8fc5cd61a5847aaab73adef8ab16723a4ea100dac3bfe` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T15-review-altitude.md | 4426 | 60 | `9331e68fcbe063cd65d1b01969bed6b6590b7ea8e4e9d25e14d72415b3cd843a` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18a1-audit-judge.md | 14903 | 80 | `1216b8e0f21ad1cd45e3db67ebb6001a24ce48c8ea7033ae461cc0936b96c3a5` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18a1-review-altitude.md | 9458 | 131 | `3716cd7b097f37aa720637ff73277f488561232498811229f236c82bc29398a6` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18a2-audit-judge.md | 16792 | 87 | `efcd9c71f26b9ecf001a44b85004354aa4a91045fc1339f732f1d79435988e02` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18a2-review-altitude.md | 12293 | 156 | `d5b421532a3b73149e1fb73bd4740431a14d0d24335c1350d221d81580b760c7` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18b-audit-judge.md | 10037 | 61 | `568676d21ad5000b75f8457ac98cbe5b64b69fb738c10c48bd6ffd660ae0e8f8` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T18b-review-altitude.md | 8021 | 104 | `ebea89ae11890ff69b2970c180f182852b586f1b48809a016e3b1512da9b2895` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T19-review-altitude.md | 8524 | 116 | `33aee565aaeb5e28f869da339adb94cc898fbfc981bd890677575750d040f3ef` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T23-review-altitude.md | 10323 | 143 | `be9d2cf360b8548ca14098b7522ecf42d82417929dc8042c6a086808b6fa49de` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T27-review-altitude.md | 10718 | 146 | `c3e8d0e331d56b52f538095dce1dfcd358ed3f403363def4a8f3a98d16847a61` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T28-review-altitude.md | 5757 | 87 | `0687ac5e6e35316f2a883e84ce5c5c9889ba391d5340bbfa33fc4c06141bcf0b` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T29-review-altitude.md | 7390 | 106 | `4db6a35662d00c3da6b2b8638db32d039fb447c2e48f7cbc443db7db678b9718` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T30-review-altitude.md | 6377 | 91 | `ef6fe54f429155c961918ddee2d01f4267df94ab57b884e7e54325af335bffa7` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T31-review-altitude.md | 8451 | 117 | `9d5510f7090e8d6735ae61ef115381c7ab290c21f379f573920e18281cc71683` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T34-review-judge.md | 18808 | 96 | `5506ad7d3252229619a4c73495511b213f135f172268b6fde0427bcd9b671594` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T37-review-judge.md | 10926 | 44 | `49c3c748f59c15805b9d62cdf2a704b67671bd23cde269222c000c59e5a5d0d4` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T38-review-altitude.md | 6100 | 80 | `6f44b52bb1e720855fa4303ef47a180070b50baf73577d50d2e92038ce389101` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T38-review-judge.md | 10449 | 32 | `46bde1d6f0251c58687f9642911fb3cc6c7ead5009ddf1f73fcfcadaa0f1f32c` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T49-review-judge.md | 3853 | 29 | `19efc29acd12b58dfecf1dff475c1a521f7cf1113740adacd61f243cabbcabfb` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T50-review-judge.md | 5959 | 34 | `08866f4c8b017e8171b555cbe9058709d11288448dba845d41020e35814e788f` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T60-review-altitude.md | 5174 | 72 | `7a1b258deb2cdfc01382800fa5cf0b540d83523385509103c084424bfb1e530c` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T60-review-judge.md | 7635 | 31 | `f6828cf5831303cf0f9c0114e776a905de0eb65701b6bb71c43a982014c33bbb` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T7-review-judge.md | 13083 | 85 | `02bc43a2ccc627f0d23f4512bcfc50817ae497a87461b6e604f94fc18bc608b8` |
| /home/mstie/projects/taurjob/docs/wave-2/reviews/T8-review-judge.md | 11826 | 79 | `4dcc29ae45ded5fc039d4a45509476ae6cdae965b078569c7a9f802bc897770f` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/altitude-reviewer.md | 8849 | 101 | `45224540b3b79da638933bb7602603c74e5141ecc066360c1012afa39ab0a08f` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/architect.md | 11436 | 49 | `8e64225868700b704c2587291dd22b170ccf14fad64950f2b1402ff7e6502cad` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/asset-generator.md | 10027 | 115 | `b3f147880272dd488e57074f0389c32bffdc4cfa60dc8df3e7d394e7f7fd0e8a` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/design-lead.md | 8168 | 91 | `304322b05853260e1e834c845a7e3ab239db8e94797da8b1a67e2c859c8f7cf4` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/heavy-implementer-1.md | 8615 | 36 | `1392b8963d233125de06505f90c530d245d832b52d6a29b91650fc879409a246` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/heavy-implementer-2.md | 9580 | 36 | `895fe883789a9f1052d47dd2e5d67b660e3d499610fa06aaf966b17f3e25db24` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/judge-astra.md | 10270 | 41 | `bf9f3e7cdc381f7a07aee27cf99a428ce538255e6262a43708bc5e33709da4cf` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/lead-taurjob.md | 8264 | 109 | `36e6ce4b7c75704086d49eaa92c51bc2b6f066d76cdcc34ec3c0a8f3bfda7aea` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/summary.md | 11819 | 149 | `1e7bfc58540dce772868302a1b4c2b2a641afbc5197e2a244c3847b5e9a89cad` |
| /home/mstie/projects/taurjob/docs/wave-2/concept-review/ui-implementer.md | 8642 | 106 | `db4c2abf8eb42ab27d7601b8930ef8535f6ab4c1c949201134b8d46008f4507e` |
| /home/mstie/projects/taurjob/docs/wave-2/imagery/gate/NOTES.md | 33511 | 340 | `446990ea0326c7c1e9842756b4382557a659e64912b004b837a3f9f7060f773e` |
| /home/mstie/projects/taurjob/docs/wave-2/imagery/set/NOTES.md | 30870 | 263 | `743e5c5e0f59ce8e666248d3c42b4e5ebb94eb493568724031db7e028121c257` |
| /home/mstie/projects/taurjob/docs/wave-2/ledger.md | 85222 | 359 | `fcb2271ffab2229f641cacd6535bef8fbe860a4428bc99ca822194f2511ad4de` |
