export NOIR_CHECKOUT ?= $(CURDIR)/../noir
CARGO ?= cargo
FIELD ?= bn254

.PHONY: test sweep compare ledger ledger-check

test:
	$(CARGO) test --locked
	$(CARGO) test --locked --features goldilocks

sweep:
	@case "$(FIELD)" in bn254|goldilocks) ;; *) echo "FIELD must be bn254 or goldilocks" >&2; exit 1 ;; esac
	$(CARGO) test --locked --lib $(if $(filter goldilocks,$(FIELD)),--features goldilocks) ledger::dump_ledger -- --exact --ignored --nocapture

compare:
	$(CARGO) test --locked --lib ledger::cross_field_diff -- --exact --ignored --nocapture

ledger:
	$(MAKE) sweep FIELD=bn254
	$(MAKE) sweep FIELD=goldilocks
	$(MAKE) compare

ledger-check: ledger
	git diff --exit-code --stat HEAD -- ledger/
