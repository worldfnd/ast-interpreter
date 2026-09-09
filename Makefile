export NOIR_CHECKOUT ?= $(CURDIR)/../noir
CARGO ?= cargo
FIELD ?= bn254

.PHONY: test sweep render status status-check

test:
	$(CARGO) test --locked
	$(CARGO) test --locked --features goldilocks

# `FIELD` names the swept field explicitly; the build still has to be linked against it, because
# `Prover.toml` inputs reach the interpreter through the ABI parser and that parser reads them in
# the linked field. The sweep asserts the two agree.
sweep:
	@case "$(FIELD)" in bn254|goldilocks) ;; *) echo "FIELD must be bn254 or goldilocks" >&2; exit 1 ;; esac
	FIELD=$(FIELD) $(CARGO) test --locked --lib $(if $(filter goldilocks,$(FIELD)),--features goldilocks) status::dump_records -- --exact --ignored --nocapture

render:
	$(CARGO) test --locked --lib status::render_status_file -- --exact --ignored --nocapture

status:
	$(MAKE) sweep FIELD=bn254
	$(MAKE) sweep FIELD=goldilocks
	$(MAKE) render

status-check: status
	git diff --exit-code --stat HEAD -- STATUS.md
