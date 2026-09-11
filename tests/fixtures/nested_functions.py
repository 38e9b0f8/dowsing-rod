# Nested function patterns.

def make_counter(start=0):
    count = start
    def increment():
        nonlocal count
        count += 1
        return count
    return increment


def make_accumulator(initial=0):
    total = initial
    def add(value):
        nonlocal total
        total += value
        return total
    return add


def create_validator(rules):
    def validate(data):
        errors = []
        for rule in rules:
            result = rule(data)
            if not result:
                errors.append(str(rule))
        return len(errors) == 0, errors
    return validate


def create_checker(checks):
    def check(item):
        failures = []
        for chk in checks:
            outcome = chk(item)
            if not outcome:
                failures.append(str(chk))
        return len(failures) == 0, failures
    return check
