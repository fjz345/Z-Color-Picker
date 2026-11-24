#include "Curves/CurveFloat.h"
#include "Curves/CurveLinearColor.h"
#include "Curves/RichCurve.h"
#include "Factories/CurveFactory.h"
#include "IAssetTools.h"
#include "Misc/AutomationTest.h"
#include "Misc/Guid.h"
#include "Misc/PackageName.h"
#include "ObjectTools.h"
#include "UObject/UObjectGlobals.h"
#include "ZColorPickerBridgeLibrary.h"

#include <limits>

#if WITH_DEV_AUTOMATION_TESTS

namespace
{
	constexpr float ColorTolerance = 1e-4f;

	UCurveLinearColor* FindCurve(const FString& AssetPath)
	{
		const FString ObjectPath = AssetPath + TEXT(".") + FPackageName::GetLongPackageAssetName(AssetPath);
		return Cast<UCurveLinearColor>(StaticFindObject(UCurveLinearColor::StaticClass(), nullptr, *ObjectPath));
	}

	bool HasKeyCountInEveryChannel(const UCurveLinearColor& Curve, int32 KeyCount)
	{
		for (const FRichCurve& ChannelCurve : Curve.FloatCurves)
		{
			if (ChannelCurve.GetNumKeys() != KeyCount)
			{
				return false;
			}
		}
		return true;
	}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FZColorPickerApplyGradientTest, "ZColorPicker.ApplyGradient", EAutomationTestFlags::EditorContext | EAutomationTestFlags::ProductFilter)

bool FZColorPickerApplyGradientTest::RunTest(const FString& Parameters)
{
	const FString Folder = TEXT("/Game/ZColorPickerTests/");
	const FString RunId = FGuid::NewGuid().ToString();
	const FString AssetPath = Folder + TEXT("Gradient_") + RunId;

	TArray<FLinearColor> Samples;
	for (int32 Index = 0; Index < 64; ++Index)
	{
		const float T = Index / 63.0f;
		Samples.Add(FLinearColor(T * T, 1.0f - T, 0.25f + 0.5f * T, 1.0f));
	}
	TestEqual(TEXT("Create returns no error"), *UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, Samples), TEXT(""));
	UCurveLinearColor* Curve = FindCurve(AssetPath);
	if (!TestNotNull(TEXT("Created curve"), Curve))
	{
		return false;
	}
	TestTrue(TEXT("64 keys per channel"), HasKeyCountInEveryChannel(*Curve, 64));
	TestTrue(TEXT("Created color at 0"), Curve->GetLinearColorValue(0.0f).Equals(Samples[0], ColorTolerance));
	TestTrue(TEXT("Created color at 0.5"), Curve->GetLinearColorValue(0.5f).Equals(FMath::Lerp(Samples[31], Samples[32], 0.5f), ColorTolerance));
	TestTrue(TEXT("Created color at 1"), Curve->GetLinearColorValue(1.0f).Equals(Samples[63], ColorTolerance));

	const TArray<FLinearColor> Update = {FLinearColor(1.0f, 0.0f, 0.0f, 1.0f), FLinearColor(0.0f, 0.0f, 1.0f, 0.5f), FLinearColor(0.0f, 1.0f, 0.0f, 0.0f)};
	TestEqual(TEXT("Update returns no error"), *UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, Update), TEXT(""));
	TestSamePtr(TEXT("Update keeps the object"), FindCurve(AssetPath), Curve);
	TestTrue(TEXT("3 keys per channel"), HasKeyCountInEveryChannel(*Curve, 3));
	TestTrue(TEXT("Updated color at 0"), Curve->GetLinearColorValue(0.0f).Equals(Update[0], ColorTolerance));
	TestTrue(TEXT("Updated color at 0.5"), Curve->GetLinearColorValue(0.5f).Equals(Update[1], ColorTolerance));
	TestTrue(TEXT("Updated color at 1"), Curve->GetLinearColorValue(1.0f).Equals(Update[2], ColorTolerance));

	const FLinearColor Single(0.3f, 0.6f, 0.9f, 1.0f);
	TestEqual(TEXT("Single color returns no error"), *UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, {Single}), TEXT(""));
	TestTrue(TEXT("1 key per channel"), HasKeyCountInEveryChannel(*Curve, 1));
	TestTrue(TEXT("Single key at 0"), Curve->FloatCurves[0].GetFirstKey().Time == 0.0f);
	TestTrue(TEXT("Single color at 0.5"), Curve->GetLinearColorValue(0.5f).Equals(Single, ColorTolerance));

	const TArray<FLinearColor> Valid = {Single};
	for (const TCHAR* BadPath : {TEXT("NotAPath"), TEXT("/Game/ZColorPickerTests/Bad Name"), TEXT("/Game/ZColorPickerTests/"), TEXT("/Engine/ZColorPickerTests/Gradient"), TEXT("/Game/ZColorPickerTests/Bad.Name")})
	{
		TestFalse(*FString::Printf(TEXT("Rejects path '%s'"), BadPath), UZColorPickerBridgeLibrary::ApplyGradient(BadPath, Valid).IsEmpty());
	}
	TestFalse(TEXT("Rejects no colors"), UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, {}).IsEmpty());
	TestFalse(TEXT("Rejects NaN"), UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, {FLinearColor(std::numeric_limits<float>::quiet_NaN(), 0.0f, 0.0f, 1.0f)}).IsEmpty());
	TestFalse(TEXT("Rejects infinity"), UZColorPickerBridgeLibrary::ApplyGradient(AssetPath, {FLinearColor(0.0f, 0.0f, 0.0f, std::numeric_limits<float>::infinity())}).IsEmpty());
	TestTrue(TEXT("Rejected calls keep the curve"), HasKeyCountInEveryChannel(*Curve, 1) && Curve->GetLinearColorValue(0.0f).Equals(Single, ColorTolerance));

	UObject* FloatCurve = IAssetTools::Get().CreateAsset(TEXT("Float_") + RunId, FPackageName::GetLongPackagePath(AssetPath), UCurveFloat::StaticClass(), NewObject<UCurveFloatFactory>());
	if (TestNotNull(TEXT("Float curve fixture"), FloatCurve))
	{
		TestFalse(TEXT("Rejects another asset class"), UZColorPickerBridgeLibrary::ApplyGradient(Folder + TEXT("Float_") + RunId, Valid).IsEmpty());
	}

	TArray<UObject*> Created = {Curve};
	if (FloatCurve)
	{
		Created.Add(FloatCurve);
	}
	ObjectTools::DeleteObjectsUnchecked(Created);
	TestNull(TEXT("Test curve deleted"), FindCurve(AssetPath));
	return true;
}

#endif
