# Stripe payment provider implementation.


def charge(amount, currency, customer_id):
    """Process a payment via Stripe."""
    validated = validate_amount(amount)
    request = build_charge_request(validated, currency, customer_id)
    response = stripe_client.charges.create(request)
    if response.status != "succeeded":
        raise PaymentError(f"Stripe charge failed: {response.failure_message}")
    result = normalize_charge_result(response)
    audit_log.record("charge", "stripe", result)
    return result


def refund(transaction_id, amount=None):
    """Issue a refund via Stripe."""
    transaction = lookup_transaction(transaction_id)
    refund_amount = amount or transaction.amount
    request = build_refund_request(transaction.provider_id, refund_amount)
    response = stripe_client.refunds.create(request)
    if response.status != "succeeded":
        raise PaymentError(f"Stripe refund failed: {response.failure_message}")
    result = normalize_refund_result(response)
    audit_log.record("refund", "stripe", result)
    return result


def get_status(transaction_id):
    """Check transaction status via Stripe."""
    transaction = lookup_transaction(transaction_id)
    response = stripe_client.charges.retrieve(transaction.provider_id)
    status = map_stripe_status(response.status)
    return {"transaction_id": transaction_id, "status": status, "provider": "stripe"}
