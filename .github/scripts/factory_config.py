"""Settings of the factory tick, in one place so a pass never holds a literal.

    RETRY_BUDGET   fix attempts on one ticket's PR before the tick stops and asks
                   (docs/design/factory-orchestrator.md Phase A, section 3 step 3)
"""
RETRY_BUDGET = 3
