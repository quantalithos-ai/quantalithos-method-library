# Method Library Evidence Index

Date: 2026-05-27
Phase: PH-08 acceptance packaging

## Evidence Map

| Evidence | Primary TC IDs | Archive |
|---|---|---|
| EV-001 | TC-DOM-001~006, TC-STATE-001 | `unit/EV-001.md` |
| EV-002 | TC-CMD-001~003 | `application/EV-002.md` |
| EV-003 | TC-DOM-003~004, TC-CMD-003/005/006/010, TC-QRY-006, TC-STATE-002/004 | `negative/EV-003.md` |
| EV-004 | TC-DOM-005~006, TC-CMD-004/009, TC-JOB-003, TC-QRY-004 | `fingerprint/EV-004.md` |
| EV-005 | TC-TX-001~002, TC-WRK-002, AC-TX-001~004 | `consistency/EV-005.md` |
| EV-006 | TC-QRY-003~006, AC-P0-006~007 | `query/EV-006.md` |
| EV-007 | TC-QRY-001~002/007, TC-JOB-004, AC-BND-005 | `projection/EV-007.md` |
| EV-008 | TC-IDEM-001~004, AC-ST-003, AC-TX-004 | `idempotency/EV-008.md` |
| EV-009 | TC-JOB-001/005, AC-JOB-001, AC-OBS-005 | `operations/EV-009.md` |
| EV-010 | TC-SYNC-001~006, TC-EVT-001, TC-WRK-001, TC-STATE-003 | `sync/EV-010.md` |
| EV-011 | TC-JOB-002~003, TC-WRK-003, AC-SYNC-007 | `recovery/EV-011.md` |
| EV-012 | AC-NF-001~005, GATE-T-08 | `nonfunctional/EV-012.md` |

## Gate Traceability

| Gate | Backing Evidence |
|---|---|
| GATE-T-01 | EV-001, EV-002, EV-003 |
| GATE-T-02 | EV-001 |
| GATE-T-03 | EV-002, EV-003, EV-004 |
| GATE-T-04 | EV-005, EV-008 |
| GATE-T-05 | EV-003, EV-006, EV-007 |
| GATE-T-06 | EV-009, EV-011 |
| GATE-T-07 | EV-010, EV-011 |
| GATE-T-08 | EV-012 |
| GATE-I-08 | EV-001~EV-012 plus `acceptance/veto-checklist.md` |

## Notes

- Each evidence report identifies its primary suites and re-run commands.
- The acceptance checklist and handoff package consume this index directly.
