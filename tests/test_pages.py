import pytest

from main import PageSelectionError, parse_page_selection, validate_pages


def test_parse_single_pages():
    result = parse_page_selection("1,3,5")
    assert result.pages == [1, 3, 5]
    assert result.groups == [[1], [3], [5]]


def test_parse_ranges():
    result = parse_page_selection("1-3,5-6")
    assert result.pages == [1, 2, 3, 5, 6]
    assert result.groups == [[1, 2, 3], [5, 6]]


def test_parse_mixed_and_duplicates():
    result = parse_page_selection("1-3,2,3,5")
    assert result.pages == [1, 2, 3, 5]
    assert result.groups == [[1, 2, 3], [2], [3], [5]]


def test_parse_invalid_range():
    with pytest.raises(PageSelectionError):
        parse_page_selection("3-1")


def test_parse_invalid_number():
    with pytest.raises(PageSelectionError):
        parse_page_selection("a")


def test_validate_pages_out_of_range():
    with pytest.raises(PageSelectionError):
        validate_pages([1, 4], total_pages=3)


def test_validate_pages_ok():
    assert validate_pages([1, 2], total_pages=2) == [1, 2]
