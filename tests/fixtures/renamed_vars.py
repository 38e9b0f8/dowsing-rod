# Functions identical except for variable and parameter names.
# Should be detected as near-duplicates under balanced normalization.

def calculate_total(price, quantity, tax_rate):
    subtotal = price * quantity
    tax_amount = subtotal * tax_rate
    total = subtotal + tax_amount
    if total < 0:
        raise ValueError("Total cannot be negative")
    return round(total, 2)


def compute_cost(unit_cost, count, vat_percentage):
    base = unit_cost * count
    vat = base * vat_percentage
    final_cost = base + vat
    if final_cost < 0:
        raise ValueError("Total cannot be negative")
    return round(final_cost, 2)


def get_amount(value, num_items, surcharge_rate):
    base_amount = value * num_items
    surcharge = base_amount * surcharge_rate
    result = base_amount + surcharge
    if result < 0:
        raise ValueError("Total cannot be negative")
    return round(result, 2)
