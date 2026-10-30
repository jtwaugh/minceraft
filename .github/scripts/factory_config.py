"""Settings of the factory tick, in one place so a pass never holds a literal.

    RETRY_BUDGET   fix attempts on one ticket's PR before the tick stops and asks
                   (docs/design/factory-orchestrator.md Phase A, section 3 step 3)
    NEEDS_YOU_ALERT_HOURS
                   hours an item may wait on the maintainer before the tick marks
                   it overdue and comments on the queue issue
                   (docs/design/factory-orchestrator.md Phase E)
"""
RETRY_BUDGET = 3
NEEDS_YOU_ALERT_HOURS = 72
