# SPEC

## backend/tests/vote_transitions.rs

### test unverified to active
Marks an unverified machine as active after it receives an exists vote.
Verifies the trust score increment and exists vote count in the summary.

### test active to suspected removed
Demotes an active machine to suspected_removed after repeated removed votes.
Confirms the resulting trust score matches the removed-vote penalty.

### test suspected removed to removed
Finalizes a suspected_removed machine as removed when enough recent removed votes exist.
Ensures stale exists votes do not prevent the removal transition.

### test removed to active revival
Revives a removed machine back to active after enough exists votes are recorded.

### test trust score floor
Clamps the trust score at zero even if removed votes would push it negative.

### test trust score ceiling
Clamps the trust score at one hundred even if exists votes would push it higher.

### test duplicate vote returns already voted
Rejects a second vote from the same fingerprint and surfaces the ALREADY_VOTED conflict code.

### test rate limit detection
Counts recent votes by IP prefix so rate limiting can trigger after repeated submissions.
