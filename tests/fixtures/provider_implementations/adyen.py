# Adyen payment provider implementation.


def charge(amount, currency, customer_id):
    """Process a payment via Adyen."""
    validated = validate_amount(amount)
    request = build_charge_request(validated, currency, customer_id)
    response = adyen_client.checkout.payments(request)
    if response.status != "Authorised":
        raise PaymentError(f"Adyen charge failed: {response.refusal_reason}")
    result = normalize_charge_result(response)
    audit_log.record("charge", "adyen", result)
    return result


def refund(transaction_id, amount=None):
    """Issue a refund via Adyen."""
    transaction = lookup_transaction(transaction_id)
    refund_amount = amount or transaction.amount
    request = build_refund_request(transaction.provider_id, refund_amount)
    response = adyen_client.checkout.refunds(request)
    if response.status != "received":
        raise PaymentError(f"Adyen refund failed: {response.refusal_reason}")
    result = normalize_refund_result(response)
    audit_log.record("refund", "adyen", result)
    return result


def get_status(transaction_id):
    """Check transaction status via Adyen."""
    transaction = lookup_transaction(transaction_id)
    response = adyen_client.checkout.get_payment(transaction.provider_id)
    status = map_adyen_status(response.status)
    return {"transaction_id": transaction_id, "status": status, "provider": "adyen"}
