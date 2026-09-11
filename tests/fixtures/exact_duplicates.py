# Exact duplicate functions — these should always be detected.

def process_order(order_id, customer):
    """Process an order for the given customer."""
    validated = validate_input(order_id, customer)
    if not validated:
        raise ValueError("Invalid order data")
    record = create_record(order_id, customer)
    result = save_to_database(record)
    send_notification(customer, result)
    return result


def handle_order(order_id, customer):
    """Handle an order for the given customer."""
    validated = validate_input(order_id, customer)
    if not validated:
        raise ValueError("Invalid order data")
    record = create_record(order_id, customer)
    result = save_to_database(record)
    send_notification(customer, result)
    return result
